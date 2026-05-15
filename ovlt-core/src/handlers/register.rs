use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use validator::Validate;

use crate::{
    error::{validation_to_app_error, AppError},
    extractors::TenantDb,
    services::{
        audit_service, email_service, one_time_token_service, password_policy_service,
        tenant_settings_service, user_service,
    },
    state::AppState,
};

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct RegisterRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1, max = 128))]
    pub password: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RegisterResponse {
    pub id: String,
    pub email: String,
    pub created_at: String,
}

#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered", body = RegisterResponse),
        (status = 409, description = "Email already exists"),
        (status = 422, description = "Validation error"),
    ),
    params(
        ("X-Tenant-ID" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn register(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    db: TenantDb,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload.validate().map_err(validation_to_app_error)?;

    let TenantDb {
        txn,
        tenant_id,
        tenant_key,
    } = db;

    // Settings and policy reads go through the RLS-scoped transaction.
    let settings = tenant_settings_service::get(&txn, tenant_id).await?;
    if !settings.allow_public_registration {
        return Err(AppError::Forbidden);
    }

    let policy = password_policy_service::get(&txn, tenant_id).await?;
    password_policy_service::validate(&payload.password, &policy)?;

    let email_normalized = payload.email.trim().to_lowercase();

    let email_lookup = hefesto::hash_for_lookup(&email_normalized, &tenant_key)?;
    let email_encrypted = hefesto::encrypt(
        &email_normalized,
        &tenant_key,
        &state.config.master_encryption_key,
    )?;
    let password_hash = hefesto::hash_password(&payload.password)?;

    if user_service::email_lookup_exists(&txn, &email_lookup).await? {
        return Err(AppError::Conflict);
    }

    let user = user_service::create(
        &txn,
        user_service::CreateUserInput {
            tenant_id,
            email_encrypted,
            email_lookup,
            password_hash,
        },
    )
    .await?;

    txn.commit().await?;

    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(
            tenant_id,
            Some(user.id),
            "user.registered",
            serde_json::json!({"ip": addr.ip().to_string()}),
        ),
    );

    if settings.require_email_verified {
        let otp = one_time_token_service::generate_otp();
        one_time_token_service::store_otp(&state.db, tenant_id, user.id, &otp, 24).await?;

        let html = format!(
            "<p>Welcome! Your email verification code is:</p>\
             <p style=\"font-size:2em;letter-spacing:0.3em\"><strong>{otp}</strong></p>\
             <p>This code expires in 24 hours.</p>"
        );
        let text = format!("Your email verification code: {otp}\nExpires in 24 hours.");

        email_service::try_send(
            &state.db,
            tenant_id,
            &tenant_key,
            &state.config.master_encryption_key,
            &email_normalized,
            "Verify your email",
            &html,
            &text,
        )
        .await;
    }

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            id: user.id.to_string(),
            email: email_normalized,
            created_at: user.created_at.to_rfc3339(),
        }),
    ))
}
