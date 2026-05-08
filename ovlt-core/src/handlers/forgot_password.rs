use axum::{extract::State, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use validator::Validate;

use crate::{
    db,
    entity::one_time_tokens,
    error::{validation_to_app_error, AppError},
    middleware::tenant::TenantContext,
    services::{email_service, one_time_token_service, user_service},
    state::AppState,
};

const RESET_EXPIRY_MINUTES: i64 = 60;

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct ForgotPasswordRequest {
    #[validate(email)]
    pub email: String,
}

/// Public endpoint — always 200 to prevent user enumeration.
/// Generates a reset token but does NOT deliver it.
/// The developer retrieves the token via GET /admin/users/:id/password-reset-token
/// and delivers it through their own channel.
#[utoipa::path(
    post,
    path = "/auth/forgot-password",
    tag = "auth",
    request_body = ForgotPasswordRequest,
    responses(
        (status = 200, description = "Reset token generated (always 200 to prevent enumeration)"),
    ),
    params(
        ("X-Tenant-ID" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn forgot_password(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<ForgotPasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload.validate().map_err(validation_to_app_error)?;

    let email_normalized = payload.email.trim().to_lowercase();
    let email_lookup = hefesto::hash_for_lookup(&email_normalized, &ctx.tenant_key)?;

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;
    let user = user_service::find_by_email_lookup(&txn, &email_lookup).await?;
    txn.commit().await?;

    if let Some(user) = user {
        if user.is_active {
            let token = one_time_token_service::generate();
            let token_hash = one_time_token_service::hash(&token);
            one_time_token_service::store(
                &state.db,
                ctx.tenant_id,
                user.id,
                token_hash,
                one_time_tokens::TYPE_PASSWORD_RESET,
                RESET_EXPIRY_MINUTES,
            )
            .await?;

            let email_plain = hefesto::decrypt(
                &user.email,
                &ctx.tenant_key,
                &state.config.master_encryption_key,
            )
            .unwrap_or_default();

            let reset_link = format!(
                "{}/auth/reset-password?token={}",
                state.config.ovlt_issuer, token
            );
            let html = format!(
                "<p>You requested a password reset.</p>\
                 <p><a href=\"{reset_link}\">Reset your password</a></p>\
                 <p>Or use this token: <code>{token}</code></p>\
                 <p>This link expires in {RESET_EXPIRY_MINUTES} minutes.</p>"
            );
            let text = format!(
                "Password reset token: {token}\n\
                 Reset link: {reset_link}\n\
                 Expires in {RESET_EXPIRY_MINUTES} minutes."
            );

            email_service::try_send(
                &state.db,
                ctx.tenant_id,
                &ctx.tenant_key,
                &state.config.master_encryption_key,
                &email_plain,
                "Reset your password",
                &html,
                &text,
            )
            .await;
        }
    }

    Ok(Json(
        serde_json::json!({ "message": "if that email exists, a reset token has been generated" }),
    ))
}
