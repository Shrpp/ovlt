use axum::{extract::State, response::IntoResponse, Extension, Json};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};

use crate::{
    entity::password_policies,
    error::AppError,
    middleware::tenant::TenantContext,
    services::{password_policy_service, tenant_settings_service},
    state::AppState,
};

// ── Password policy ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PolicyResponse {
    pub min_length: i32,
    pub require_uppercase: bool,
    pub require_digit: bool,
    pub require_special: bool,
    pub history_size: i32,
}

pub async fn get_policy(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, AppError> {
    match password_policies::Entity::find_by_id(ctx.tenant_id)
        .one(&state.db)
        .await?
    {
        Some(p) => Ok(Json(PolicyResponse {
            min_length: p.min_length,
            require_uppercase: p.require_uppercase,
            require_digit: p.require_digit,
            require_special: p.require_special,
            history_size: p.history_size,
        })),
        None => Ok(Json(PolicyResponse {
            min_length: 8,
            require_uppercase: false,
            require_digit: false,
            require_special: false,
            history_size: 0,
        })),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpsertPolicyRequest {
    pub min_length: i32,
    pub require_uppercase: bool,
    pub require_digit: bool,
    pub require_special: bool,
    pub history_size: i32,
}

pub async fn put_policy(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(req): Json<UpsertPolicyRequest>,
) -> Result<impl IntoResponse, AppError> {
    if req.min_length < 1 || req.min_length > 128 {
        return Err(AppError::InvalidInput(
            "min_length must be between 1 and 128".into(),
        ));
    }
    password_policy_service::upsert(
        &state.db,
        ctx.tenant_id,
        req.min_length,
        req.require_uppercase,
        req.require_digit,
        req.require_special,
        req.history_size,
    )
    .await?;
    Ok(Json(
        serde_json::json!({ "message": "password policy saved" }),
    ))
}

// ── Lockout policy ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct LockoutResponse {
    pub max_attempts: i32,
    pub window_minutes: i32,
    pub duration_minutes: i32,
}

pub async fn get_lockout(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, AppError> {
    let s = tenant_settings_service::get(&state.db, ctx.tenant_id).await?;
    Ok(Json(LockoutResponse {
        max_attempts: s.lockout_max_attempts as i32,
        window_minutes: s.lockout_window_minutes as i32,
        duration_minutes: s.lockout_duration_minutes as i32,
    }))
}

#[derive(Debug, Deserialize)]
pub struct UpsertLockoutRequest {
    pub max_attempts: i32,
    pub window_minutes: i32,
    pub duration_minutes: i32,
}

pub async fn put_lockout(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(req): Json<UpsertLockoutRequest>,
) -> Result<impl IntoResponse, AppError> {
    if req.max_attempts < 1 || req.window_minutes < 1 || req.duration_minutes < 1 {
        return Err(AppError::InvalidInput(
            "all lockout values must be >= 1".into(),
        ));
    }
    let s = tenant_settings_service::get(&state.db, ctx.tenant_id).await?;
    tenant_settings_service::upsert(
        &state.db,
        ctx.tenant_id,
        req.max_attempts,
        req.window_minutes,
        req.duration_minutes,
        s.access_token_ttl_minutes as i32,
        s.refresh_token_ttl_days as i32,
        s.allow_public_registration,
        s.require_email_verified,
    )
    .await?;
    Ok(Json(
        serde_json::json!({ "message": "lockout policy saved" }),
    ))
}

// ── Token TTL ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TokenTtlResponse {
    pub access_token_ttl_minutes: i32,
    pub refresh_token_ttl_days: i32,
}

pub async fn get_token_ttl(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, AppError> {
    let s = tenant_settings_service::get(&state.db, ctx.tenant_id).await?;
    Ok(Json(TokenTtlResponse {
        access_token_ttl_minutes: s.access_token_ttl_minutes as i32,
        refresh_token_ttl_days: s.refresh_token_ttl_days as i32,
    }))
}

#[derive(Debug, Deserialize)]
pub struct UpsertTokenTtlRequest {
    pub access_token_ttl_minutes: i32,
    pub refresh_token_ttl_days: i32,
}

pub async fn put_token_ttl(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(req): Json<UpsertTokenTtlRequest>,
) -> Result<impl IntoResponse, AppError> {
    if req.access_token_ttl_minutes < 1 || req.refresh_token_ttl_days < 1 {
        return Err(AppError::InvalidInput("TTL values must be >= 1".into()));
    }
    let s = tenant_settings_service::get(&state.db, ctx.tenant_id).await?;
    tenant_settings_service::upsert(
        &state.db,
        ctx.tenant_id,
        s.lockout_max_attempts as i32,
        s.lockout_window_minutes as i32,
        s.lockout_duration_minutes as i32,
        req.access_token_ttl_minutes,
        req.refresh_token_ttl_days,
        s.allow_public_registration,
        s.require_email_verified,
    )
    .await?;
    Ok(Json(serde_json::json!({ "message": "token TTL saved" })))
}

// ── Registration policy ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct RegistrationResponse {
    pub allow_public_registration: bool,
    pub require_email_verified: bool,
}

pub async fn get_registration(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, AppError> {
    let s = tenant_settings_service::get(&state.db, ctx.tenant_id).await?;
    Ok(Json(RegistrationResponse {
        allow_public_registration: s.allow_public_registration,
        require_email_verified: s.require_email_verified,
    }))
}

#[derive(Debug, Deserialize)]
pub struct UpsertRegistrationRequest {
    pub allow_public_registration: bool,
    pub require_email_verified: bool,
}

pub async fn put_registration(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(req): Json<UpsertRegistrationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let s = tenant_settings_service::get(&state.db, ctx.tenant_id).await?;
    tenant_settings_service::upsert(
        &state.db,
        ctx.tenant_id,
        s.lockout_max_attempts as i32,
        s.lockout_window_minutes as i32,
        s.lockout_duration_minutes as i32,
        s.access_token_ttl_minutes as i32,
        s.refresh_token_ttl_days as i32,
        req.allow_public_registration,
        req.require_email_verified,
    )
    .await?;
    Ok(Json(
        serde_json::json!({ "message": "registration policy saved" }),
    ))
}
