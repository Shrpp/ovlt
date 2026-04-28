use axum::{extract::State, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use validator::Validate;

use crate::{
    db,
    entity::one_time_tokens,
    error::AppError,
    middleware::tenant::TenantContext,
    services::{one_time_token_service, user_service},
    state::AppState,
};

const RESET_EXPIRY_MINUTES: i64 = 60;

#[derive(Debug, Deserialize, Validate)]
pub struct ForgotPasswordRequest {
    #[validate(email)]
    pub email: String,
}

/// Public endpoint — always 200 to prevent user enumeration.
/// Generates a reset token but does NOT deliver it.
/// The developer retrieves the token via GET /admin/users/:id/password-reset-token
/// and delivers it through their own channel.
pub async fn forgot_password(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<ForgotPasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

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
        }
    }

    Ok(Json(
        serde_json::json!({ "message": "if that email exists, a reset token has been generated" }),
    ))
}
