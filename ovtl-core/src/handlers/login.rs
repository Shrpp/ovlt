use axum::{extract::State, response::IntoResponse, Extension, Json};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    db,
    error::AppError,
    middleware::tenant::TenantContext,
    services::{token_service, user_service},
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
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let email_lookup = hefesto::hash_for_lookup(&payload.email, &ctx.tenant_key);

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let user = user_service::find_by_email_lookup(&txn, &email_lookup)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !user.is_active {
        return Err(AppError::Unauthorized);
    }

    if !hefesto::verify_password(&payload.password, &user.password_hash) {
        return Err(AppError::Unauthorized);
    }

    let email_plain = hefesto::decrypt(
        &user.email,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;

    let access_token = token_service::generate_access_token(
        user.id,
        ctx.tenant_id,
        &email_plain,
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

    Ok(Json(TokenResponse {
        access_token,
        refresh_token,
        expires_in: state.config.jwt_expiration_minutes * 60,
    }))
}
