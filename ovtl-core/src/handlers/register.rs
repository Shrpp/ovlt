use axum::{extract::{ConnectInfo, State}, http::StatusCode, response::IntoResponse, Extension, Json};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use validator::Validate;

use crate::{
    db,
    error::AppError,
    middleware::tenant::TenantContext,
    services::{audit_service, user_service},
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
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    validate_password_strength(&payload.password)?;

    // Normalize email — prevents duplicate accounts differing only by case.
    let email_normalized = payload.email.trim().to_lowercase();

    let email_lookup = hefesto::hash_for_lookup(&email_normalized, &ctx.tenant_key)?;
    let email_encrypted = hefesto::encrypt(
        &email_normalized,
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

    audit_service::record(
        state.db.clone(),
        ctx.tenant_id,
        Some(user.id),
        "user.registered",
        Some(addr.ip().to_string()),
        None,
    );

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            id: user.id.to_string(),
            email: email_normalized,
            created_at: user.created_at.to_rfc3339(),
        }),
    ))
}

fn validate_password_strength(password: &str) -> Result<(), AppError> {
    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_upper || !has_digit {
        return Err(AppError::InvalidInput(
            "password must contain at least one uppercase letter and one digit".into(),
        ));
    }
    Ok(())
}
