use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db,
    error::AppError,
    handlers::admin_auth,
    middleware::{auth::AuthUser, tenant::TenantContext},
    services::{
        mfa_service, permission_service, role_service, session_service, tenant_settings_service,
        token_service, user_service,
    },
    state::AppState,
};

// ── Setup (protected) ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SetupResponse {
    pub secret: String,
    pub otpauth_uri: String,
}

#[utoipa::path(
    post,
    path = "/auth/mfa/setup",
    tag = "auth",
    responses(
        (status = 200, description = "MFA setup initiated", body = SetupResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    params(
        ("X-Tenant-ID" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn mfa_setup(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, AppError> {
    let secret = mfa_service::generate_secret();

    let secret_enc = hefesto::encrypt(
        &secret,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let user = user_service::find_by_id(&txn, auth.user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let email_plain = hefesto::decrypt(
        &user.email,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;

    mfa_service::upsert_pending(&txn, ctx.tenant_id, auth.user_id, secret_enc).await?;
    txn.commit().await?;

    let uri = mfa_service::totp_uri(&secret, &email_plain, &state.config.ovlt_issuer);

    Ok(Json(SetupResponse {
        secret,
        otpauth_uri: uri,
    }))
}

// ── Confirm (protected) ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ConfirmRequest {
    pub code: String,
}

#[utoipa::path(
    post,
    path = "/auth/mfa/confirm",
    tag = "auth",
    request_body = ConfirmRequest,
    responses(
        (status = 204, description = "MFA confirmed and activated"),
        (status = 401, description = "Invalid TOTP code"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    params(
        ("X-Tenant-ID" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn mfa_confirm(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<ConfirmRequest>,
) -> Result<impl IntoResponse, AppError> {
    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let record = mfa_service::find_any(&txn, ctx.tenant_id, auth.user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let secret = hefesto::decrypt(
        &record.secret_enc,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;

    if !mfa_service::verify_code(&secret, &payload.code) {
        txn.commit().await?;
        return Err(AppError::Unauthorized);
    }

    mfa_service::activate(&txn, ctx.tenant_id, auth.user_id).await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Disable (protected) ──────────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/auth/mfa/disable",
    tag = "auth",
    request_body = ConfirmRequest,
    responses(
        (status = 204, description = "MFA disabled"),
        (status = 401, description = "Invalid TOTP code"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    params(
        ("X-Tenant-ID" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn mfa_disable(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<ConfirmRequest>,
) -> Result<impl IntoResponse, AppError> {
    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let record = mfa_service::find_enabled(&txn, ctx.tenant_id, auth.user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let secret = hefesto::decrypt(
        &record.secret_enc,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;

    if !mfa_service::verify_code(&secret, &payload.code) {
        txn.commit().await?;
        return Err(AppError::Unauthorized);
    }

    mfa_service::disable(&txn, ctx.tenant_id, auth.user_id).await?;
    mfa_service::delete_backup_codes(&txn, ctx.tenant_id, auth.user_id).await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Backup codes (protected) ─────────────────────────────────────────────────

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct BackupCodesRequest {
    pub code: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct BackupCodesResponse {
    pub codes: Vec<String>,
}

#[utoipa::path(
    post,
    path = "/auth/mfa/backup-codes",
    tag = "auth",
    request_body = BackupCodesRequest,
    responses(
        (status = 200, description = "Backup codes generated", body = BackupCodesResponse),
        (status = 401, description = "Invalid TOTP code or MFA not enabled"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    params(
        ("X-Tenant-ID" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn mfa_backup_codes_generate(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<BackupCodesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let record = mfa_service::find_enabled(&txn, ctx.tenant_id, auth.user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let secret = hefesto::decrypt(
        &record.secret_enc,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;

    if !mfa_service::verify_code(&secret, &payload.code) {
        txn.commit().await?;
        return Err(AppError::Unauthorized);
    }

    let codes =
        mfa_service::generate_backup_codes(&txn, ctx.tenant_id, auth.user_id).await?;
    txn.commit().await?;

    Ok(Json(BackupCodesResponse { codes }))
}

// ── Challenge (public, requires mfa_token) ───────────────────────────────────

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ChallengeRequest {
    pub mfa_token: String,
    pub code: Option<String>,
    pub backup_code: Option<String>,
}

#[utoipa::path(
    post,
    path = "/auth/mfa/challenge",
    tag = "auth",
    request_body = ChallengeRequest,
    responses(
        (status = 200, description = "MFA verified, tokens issued", body = crate::handlers::login::TokenResponse),
        (status = 401, description = "Invalid MFA token or TOTP code"),
    ),
    params(
        ("X-Tenant-ID" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn mfa_challenge(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<ChallengeRequest>,
) -> Result<impl IntoResponse, AppError> {
    let claims = token_service::verify_mfa_token(
        &payload.mfa_token,
        &state.config.jwt_secret,
        state.config.jwt_secret_previous.as_deref(),
    )?;

    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized)?;
    let token_tenant_id = Uuid::parse_str(&claims.tid).map_err(|_| AppError::Unauthorized)?;

    if token_tenant_id != ctx.tenant_id {
        return Err(AppError::Unauthorized);
    }

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let record = mfa_service::find_enabled(&txn, ctx.tenant_id, user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let verified = if let Some(ref totp_code) = payload.code {
        let secret = hefesto::decrypt(
            &record.secret_enc,
            &ctx.tenant_key,
            &state.config.master_encryption_key,
        )?;
        mfa_service::verify_code(&secret, totp_code)
    } else if let Some(ref bcode) = payload.backup_code {
        mfa_service::consume_backup_code(&txn, ctx.tenant_id, user_id, bcode).await?
    } else {
        false
    };

    if !verified {
        txn.commit().await?;
        return Err(AppError::Unauthorized);
    }

    let user = user_service::find_by_id(&txn, user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !user.is_active {
        txn.commit().await?;
        return Err(AppError::Unauthorized);
    }

    let email_plain = hefesto::decrypt(
        &user.email,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;

    let settings = tenant_settings_service::get(&state.db, ctx.tenant_id).await?;

    let roles = role_service::list_names_for_user(&txn, user_id, ctx.tenant_id)
        .await
        .unwrap_or_default();
    let permissions = permission_service::list_names_for_user(&txn, user_id, ctx.tenant_id)
        .await
        .unwrap_or_default();

    let access_token = token_service::generate_access_token(
        user_id,
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
        user_id,
        token_hash,
        settings.refresh_token_ttl_days,
    )
    .await?;

    txn.commit().await?;

    let session_id = session_service::create(
        &state.db,
        ctx.tenant_id,
        user_id,
        session_service::SessionData {
            email: email_plain,
            ip: None,
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

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(axum::http::header::SET_COOKIE, cookie.parse().unwrap());

    Ok((
        headers,
        Json(crate::handlers::login::TokenResponse {
            access_token,
            refresh_token,
            expires_in: settings.access_token_ttl_minutes * 60,
        }),
    ))
}

// ── Admin disable MFA for any user ───────────────────────────────────────────

fn extract_tenant_id(headers: &HeaderMap) -> Result<Uuid, AppError> {
    headers
        .get("x-ovlt-tenant-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::InvalidInput("x-ovlt-tenant-id header required".into()))
}

#[utoipa::path(
    delete,
    path = "/users/{id}/mfa",
    tag = "admin-users",
    responses(
        (status = 204, description = "MFA disabled for user"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("id" = String, Path, description = "User UUID"),
        ("X-Tenant-ID" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn admin_disable_mfa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    admin_auth::require_admin(
        &headers,
        &state.config,
        state.master_tenant_id,
    )?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    mfa_service::disable(&txn, tenant_id, user_id).await?;
    mfa_service::delete_backup_codes(&txn, tenant_id, user_id).await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}
