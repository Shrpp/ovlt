use axum::{
    extract::{ConnectInfo, State},
    http::header,
    response::IntoResponse,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use validator::Validate;

use crate::{
    db,
    error::{validation_to_app_error, AppError},
    middleware::tenant::TenantContext,
    services::{
        audit_service, lockout_service, mfa_service, permission_service, role_service,
        session_service, tenant_settings_service, token_service, user_service,
    },
    state::AppState,
};

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct LoginRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8, max = 128))]
    pub password: String,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct MfaRequiredResponse {
    pub mfa_required: bool,
    pub mfa_token: String,
}

#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = TokenResponse),
        (status = 200, description = "MFA required", body = MfaRequiredResponse),
        (status = 401, description = "Invalid credentials"),
        (status = 422, description = "Validation error"),
    ),
    params(
        ("X-Tenant-ID" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload.validate().map_err(validation_to_app_error)?;

    let ip = addr.ip().to_string();

    let settings = tenant_settings_service::get(&state.db, ctx.tenant_id).await?;

    let email_normalized = payload.email.trim().to_lowercase();
    let email_lookup = hefesto::hash_for_lookup(&email_normalized, &ctx.tenant_key)?;

    if lockout_service::is_locked(
        &state.db,
        ctx.tenant_id,
        &email_lookup,
        settings.lockout_max_attempts,
        settings.lockout_window_minutes,
    )
    .await?
    {
        audit_service::record_best_effort(
            state.db.clone(),
            audit_service::AuditEvent::new(
                ctx.tenant_id,
                None,
                "login.locked",
                serde_json::json!({"ip": ip}),
            ),
        );
        return Err(AppError::Unauthorized);
    }

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let user = match user_service::find_by_email_lookup(&txn, &email_lookup).await? {
        Some(u) => u,
        None => {
            txn.commit().await?;
            lockout_service::record_attempt(&state.db, ctx.tenant_id, &email_lookup).await?;
            audit_service::record_best_effort(
                state.db.clone(),
                audit_service::AuditEvent::new(
                    ctx.tenant_id,
                    None,
                    "login.failed.unknown_email",
                    serde_json::json!({"ip": ip}),
                ),
            );
            return Err(AppError::Unauthorized);
        }
    };

    if !user.is_active {
        txn.commit().await?;
        return Err(AppError::Unauthorized);
    }

    if settings.require_email_verified && !user.email_verified {
        txn.commit().await?;
        return Err(AppError::InvalidInput("email not verified".into()));
    }

    if !hefesto::verify_password(&payload.password, &user.password_hash) {
        txn.commit().await?;
        lockout_service::record_attempt(&state.db, ctx.tenant_id, &email_lookup).await?;
        audit_service::record_best_effort(
            state.db.clone(),
            audit_service::AuditEvent::new(
                ctx.tenant_id,
                Some(user.id),
                "login.failed.wrong_password",
                serde_json::json!({"ip": ip}),
            ),
        );
        return Err(AppError::Unauthorized);
    }

    let email_plain = hefesto::decrypt(
        &user.email,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;

    // MFA check — if enabled, issue a short-lived challenge token instead of full tokens
    if mfa_service::find_enabled(&txn, ctx.tenant_id, user.id)
        .await?
        .is_some()
    {
        txn.commit().await?;
        lockout_service::clear_attempts(&state.db, ctx.tenant_id, &email_lookup).await?;
        let mfa_token =
            token_service::generate_mfa_token(user.id, ctx.tenant_id, &state.config.jwt_secret)?;
        return Ok(Json(json!({
            "mfa_required": true,
            "mfa_token": mfa_token,
        }))
        .into_response());
    }

    let roles = role_service::list_names_for_user(&txn, user.id, ctx.tenant_id)
        .await
        .unwrap_or_default();
    let permissions = permission_service::list_names_for_user(&txn, user.id, ctx.tenant_id)
        .await
        .unwrap_or_default();

    let access_token = token_service::generate_access_token(
        user.id,
        ctx.tenant_id,
        &email_plain,
        roles,
        permissions,
        std::collections::HashMap::new(),
        &state.config.jwt_secret,
        settings.access_token_ttl_minutes,
    )?;

    let refresh_token = token_service::generate_refresh_token();
    let token_hash = token_service::hash_refresh_token(&refresh_token);

    token_service::store_refresh_token(
        &txn,
        ctx.tenant_id,
        user.id,
        token_hash,
        settings.refresh_token_ttl_days,
    )
    .await?;

    txn.commit().await?;

    lockout_service::clear_attempts(&state.db, ctx.tenant_id, &email_lookup).await?;
    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(
            ctx.tenant_id,
            Some(user.id),
            "login.success",
            serde_json::json!({"ip": ip}),
        ),
    );

    let session_id = session_service::create(
        &state.db,
        ctx.tenant_id,
        user.id,
        session_service::SessionData {
            email: email_plain,
            ip: Some(ip),
        },
        settings.refresh_token_ttl_days,
    )
    .await
    .unwrap_or_default();

    let secure = if state.config.is_production() {
        "; Secure"
    } else {
        ""
    };
    let cookie = format!(
        "ovlt_session={session_id}; HttpOnly; SameSite=Lax{secure}; Path=/; Max-Age={}",
        settings.refresh_token_ttl_days * 86400
    );

    let mut response_headers = axum::http::HeaderMap::new();
    response_headers.insert(header::SET_COOKIE, cookie.parse().unwrap());

    Ok((
        response_headers,
        Json(TokenResponse {
            access_token,
            refresh_token,
            expires_in: settings.access_token_ttl_minutes * 60,
        }),
    )
        .into_response())
}
