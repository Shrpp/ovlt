use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    db,
    error::AppError,
    middleware::tenant::TenantContext,
    services::user_service,
    state::AppState,
};

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8, max = 128))]
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub id: String,
    pub email: String,
    pub created_at: String,
}

pub async fn register(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let email_lookup = hefesto::hash_for_lookup(&payload.email, &ctx.tenant_key);
    let email_encrypted = hefesto::encrypt(
        &payload.email,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;
    let password_hash = hefesto::hash_password(&payload.password)?;

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    if user_service::email_lookup_exists(&txn, &email_lookup).await? {
        return Err(AppError::Conflict);
    }

    let user = user_service::create(
        &txn,
        user_service::CreateUserInput {
            tenant_id: ctx.tenant_id,
            email_encrypted,
            email_lookup,
            password_hash,
        },
    )
    .await?;

    txn.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            id: user.id.to_string(),
            email: payload.email,
            created_at: user.created_at.to_rfc3339(),
        }),
    ))
}
