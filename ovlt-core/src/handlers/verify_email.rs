use axum::{response::IntoResponse, Json};
use sea_orm::{ActiveModelTrait, Set};
use serde::Deserialize;
use validator::Validate;

use crate::{
    entity::users,
    error::{validation_to_app_error, AppError},
    extractors::TenantDb,
    services::{one_time_token_service, user_service},
};

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct VerifyOtpRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 6, max = 6))]
    pub otp: String,
}

/// `POST /auth/verify-otp`
/// Accepts the 6-digit OTP the admin shared with the user.
/// Marks the user's email as verified.
#[utoipa::path(
    post,
    path = "/auth/verify-otp",
    tag = "auth",
    request_body = VerifyOtpRequest,
    responses(
        (status = 200, description = "Email verified successfully"),
        (status = 401, description = "Invalid OTP"),
        (status = 422, description = "Validation error"),
    ),
    params(
        ("X-Tenant-ID" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn verify_email(
    db: TenantDb,
    Json(payload): Json<VerifyOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload.validate().map_err(validation_to_app_error)?;

    let TenantDb {
        txn, tenant_key, ..
    } = db;

    let email_normalized = payload.email.trim().to_lowercase();
    let email_lookup = hefesto::hash_for_lookup(&email_normalized, &tenant_key)?;

    let user = user_service::find_by_email_lookup(&txn, &email_lookup)
        .await?
        .ok_or(AppError::InvalidInput("invalid OTP".into()))?;

    // OTP consumed within the tenant transaction — RLS enforces tenant isolation.
    one_time_token_service::consume_otp(&txn, user.id, &payload.otp).await?;

    let mut active: users::ActiveModel = user.into();
    active.email_verified = Set(true);
    active.update(&txn).await?;

    txn.commit().await?;

    Ok(Json(
        serde_json::json!({ "message": "email verified successfully" }),
    ))
}
