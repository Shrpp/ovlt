use axum::{extract::{ConnectInfo, State}, http::header, response::IntoResponse, Extension, Json};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use validator::Validate;

use crate::{
    db,
    error::AppError,
    middleware::tenant::TenantContext,
    services::{audit_service, lockout_service, permission_service, role_service, session_service, token_service, user_service},
    state::AppState,
};

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8, max = 128))]
    pub password: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let ip = addr.ip().to_string();

    // Normalize email before lookup — consistent with registration.
    let email_normalized = payload.email.trim().to_lowercase();
    let email_lookup = hefesto::hash_for_lookup(&email_normalized, &ctx.tenant_key)?;

    // Check account lockout before touching the DB user record.
    if lockout_service::is_locked(&state.db, ctx.tenant_id, &email_lookup).await? {
        audit_service::record(
            state.db.clone(),
            ctx.tenant_id,
            None,
            "login.locked",
            Some(ip.clone()),
            None,
        );
        return Err(AppError::Unauthorized);
    }

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let user = match user_service::find_by_email_lookup(&txn, &email_lookup).await? {
        Some(u) => u,
        None => {
            txn.commit().await?;
            lockout_service::record_attempt(&state.db, ctx.tenant_id, &email_lookup).await?;
            audit_service::record(
                state.db.clone(),
                ctx.tenant_id,
                None,
                "login.failed.unknown_email",
                Some(ip),
                None,
            );
            return Err(AppError::Unauthorized);
        }
    };

    if !user.is_active {
        txn.commit().await?;
        return Err(AppError::Unauthorized);
    }

    if !hefesto::verify_password(&payload.password, &user.password_hash) {
        txn.commit().await?;
        lockout_service::record_attempt(&state.db, ctx.tenant_id, &email_lookup).await?;
        audit_service::record(
            state.db.clone(),
            ctx.tenant_id,
            Some(user.id),
            "login.failed.wrong_password",
            Some(ip),
            None,
        );
        return Err(AppError::Unauthorized);
    }

    let email_plain = hefesto::decrypt(
        &user.email,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;

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
        &state.config.jwt_secret,
        state.config.jwt_expiration_minutes,
    )?;

    let refresh_token = token_service::generate_refresh_token();
    let token_hash = token_service::hash_refresh_token(&refresh_token);

    token_service::store_refresh_token(
        &txn,
        ctx.tenant_id,
        user.id,
        token_hash,
        state.config.refresh_token_expiration_days,
    )
    .await?;

    txn.commit().await?;

    // Clear failed attempts and record successful login.
    lockout_service::clear_attempts(&state.db, ctx.tenant_id, &email_lookup).await?;
    audit_service::record(
        state.db.clone(),
        ctx.tenant_id,
        Some(user.id),
        "login.success",
        Some(ip.clone()),
        None,
    );

    // Create SSO session.
    let session_ttl_days = state.config.refresh_token_expiration_days;
    let session_id = session_service::create(
        &state.db,
        ctx.tenant_id,
        user.id,
        session_service::SessionData { email: email_plain, ip: Some(ip) },
        session_ttl_days,
    )
    .await
    .unwrap_or_default();

    let cookie = format!(
        "ovtl_session={session_id}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}",
        session_ttl_days * 86400
    );

    let mut response_headers = axum::http::HeaderMap::new();
    response_headers.insert(header::SET_COOKIE, cookie.parse().unwrap());

    Ok((
        response_headers,
        Json(TokenResponse {
            access_token,
            refresh_token,
            expires_in: state.config.jwt_expiration_minutes * 60,
        }),
    ))
}
