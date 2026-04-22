use axum::{extract::State, response::IntoResponse, Extension, Json};
use serde::Deserialize;

use crate::{
    db,
    entity::users,
    error::AppError,
    middleware::tenant::TenantContext,
    services::token_service,
    state::AppState,
};
use sea_orm::EntityTrait;

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

pub async fn refresh(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<RefreshRequest>,
) -> Result<impl IntoResponse, AppError> {
    let token_hash = token_service::hash_refresh_token(&payload.refresh_token);

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let record = token_service::find_valid_refresh_token(&txn, &token_hash)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let user_id = record.user_id;

    // Rotation: revoke old token before issuing new one
    token_service::revoke_token(&txn, record).await?;

    let user = users::Entity::find_by_id(user_id)
        .one(&txn)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !user.is_active {
        return Err(AppError::Unauthorized);
    }

    let email = hefesto::decrypt(
        &user.email,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;

    let access_token = token_service::generate_access_token(
        user.id,
        ctx.tenant_id,
        &email,
        &state.config.jwt_secret,
        state.config.jwt_expiration_minutes,
    )?;

    let new_refresh_token = token_service::generate_refresh_token();
    let new_hash = token_service::hash_refresh_token(&new_refresh_token);

    token_service::store_refresh_token(
        &txn,
        ctx.tenant_id,
        user.id,
        new_hash,
        state.config.refresh_token_expiration_days,
    )
    .await?;

    txn.commit().await?;

    Ok(Json(super::login::TokenResponse {
        access_token,
        refresh_token: new_refresh_token,
        expires_in: state.config.jwt_expiration_minutes * 60,
    }))
}
