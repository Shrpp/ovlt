use axum::{extract::State, response::IntoResponse, Extension, Json};
use sea_orm::{ActiveModelTrait, Set};
use serde::Deserialize;
use validator::Validate;

use crate::{
    db,
    entity::users,
    error::AppError,
    middleware::tenant::TenantContext,
    services::{one_time_token_service, user_service},
    state::AppState,
};

#[derive(Debug, Deserialize, Validate)]
pub struct VerifyOtpRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 6, max = 6))]
    pub otp: String,
}

/// `POST /auth/verify-otp`
/// Accepts the 6-digit OTP the admin shared with the user.
/// Marks the user's email as verified.
pub async fn verify_email(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<VerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let email_normalized = payload.email.trim().to_lowercase();
    let email_lookup = hefesto::hash_for_lookup(&email_normalized, &ctx.tenant_key)?;

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;
    let user = user_service::find_by_email_lookup(&txn, &email_lookup)
        .await?
        .ok_or(AppError::InvalidInput("invalid OTP".into()))?; // don't reveal user existence
    txn.commit().await?;

    one_time_token_service::consume_otp(&state.db, user.id, &payload.otp).await?;

    let txn2 = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;
    let mut active: users::ActiveModel = user.into();
    active.email_verified = Set(true);
    active.update(&txn2).await?;
    txn2.commit().await?;

    Ok(Json(
        serde_json::json!({ "message": "email verified successfully" }),
    ))
}
