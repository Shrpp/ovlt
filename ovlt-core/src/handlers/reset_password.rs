use axum::{extract::State, response::IntoResponse, Extension, Json};
use sea_orm::{ActiveModelTrait, Set};
use serde::Deserialize;
use validator::Validate;

use crate::{
    db,
    entity::{one_time_tokens, users},
    error::AppError,
    middleware::tenant::TenantContext,
    services::{one_time_token_service, password_policy_service, token_service, user_service},
    state::AppState,
};

#[derive(Debug, Deserialize, Validate)]
pub struct ResetPasswordRequest {
    pub token: String,
    #[validate(length(min = 1, max = 128))]
    pub new_password: String,
}

pub async fn reset_password(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<ResetPasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    // Validate and consume the token (marks it used, returns the record).
    let record = one_time_token_service::consume(
        &state.db,
        &payload.token,
        one_time_tokens::TYPE_PASSWORD_RESET,
    )
    .await?;

    // Validate token belongs to this tenant.
    if record.tenant_id != ctx.tenant_id {
        return Err(AppError::Unauthorized);
    }

    // Apply password policy.
    let policy = password_policy_service::get(&state.db, ctx.tenant_id).await?;
    password_policy_service::validate(&payload.new_password, &policy)?;

    let new_hash = hefesto::hash_password(&payload.new_password)?;

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let user = user_service::find_by_id(&txn, record.user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut active: users::ActiveModel = user.into();
    active.password_hash = Set(new_hash);
    active.update(&txn).await?;

    // Revoke all refresh tokens so existing sessions must re-login.
    token_service::revoke_all_user_tokens(&txn, record.user_id).await?;

    txn.commit().await?;

    Ok(Json(
        serde_json::json!({ "message": "password updated successfully" }),
    ))
}
