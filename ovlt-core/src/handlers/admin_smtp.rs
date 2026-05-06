use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{
    entity::tenant_smtp_config, error::AppError, handlers::admin_auth, services::tenant_service,
    state::AppState,
};

fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    admin_auth::require_admin(
        headers,
        &state.config.admin_key,
        &state.config.jwt_secret,
        state.master_tenant_id,
    )
    .map(|_| ())
}

fn extract_tenant_id(headers: &HeaderMap) -> Result<Uuid, AppError> {
    headers
        .get("x-ovlt-tenant-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::InvalidInput("x-ovlt-tenant-id header required".into()))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SmtpConfigResponse {
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password_set: bool,
    pub from_name: String,
    pub from_email: String,
    pub use_tls: bool,
    pub enabled: bool,
    pub updated_at: String,
}

#[utoipa::path(
    get,
    path = "/admin/smtp",
    tag = "admin-smtp",
    responses(
        (status = 200, description = "Current SMTP configuration", body = SmtpConfigResponse),
        (status = 404, description = "SMTP not configured"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn get_smtp(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    match tenant_smtp_config::Entity::find_by_id(tenant_id)
        .one(&state.db)
        .await?
    {
        Some(r) => Ok(Json(SmtpConfigResponse {
            host: r.host,
            port: r.port,
            username: r.username,
            password_set: !r.password_enc.is_empty(),
            from_name: r.from_name,
            from_email: r.from_email,
            use_tls: r.use_tls,
            enabled: r.enabled,
            updated_at: r.updated_at.to_rfc3339(),
        })),
        None => Err(AppError::NotFound),
    }
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpsertSmtpRequest {
    #[validate(length(min = 1, max = 253))]
    pub host: String,
    pub port: Option<i32>,
    #[validate(length(min = 1))]
    pub username: String,
    /// Omit to keep existing password.
    pub password: Option<String>,
    #[validate(length(min = 1))]
    pub from_name: String,
    #[validate(email)]
    pub from_email: String,
    pub use_tls: Option<bool>,
    pub enabled: Option<bool>,
}

#[utoipa::path(
    put,
    path = "/admin/smtp",
    tag = "admin-smtp",
    request_body = UpsertSmtpRequest,
    responses(
        (status = 200, description = "SMTP configuration saved"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn put_smtp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UpsertSmtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    req.validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let tenant = tenant_service::find_active(&state.db, tenant_id).await?;
    let tenant_key = hefesto::decrypt(
        &tenant.encryption_key_encrypted,
        &state.config.tenant_wrap_key,
        &state.config.master_encryption_key,
    )?;

    let now = Utc::now().fixed_offset();
    let port = req.port.unwrap_or(587);
    let use_tls = req.use_tls.unwrap_or(true);
    let enabled = req.enabled.unwrap_or(false);

    let existing = tenant_smtp_config::Entity::find_by_id(tenant_id)
        .one(&state.db)
        .await?;

    let password_enc = if let Some(pw) = req.password {
        hefesto::encrypt(&pw, &tenant_key, &state.config.master_encryption_key)?
    } else if let Some(ref rec) = existing {
        rec.password_enc.clone()
    } else {
        return Err(AppError::InvalidInput(
            "password is required for initial setup".into(),
        ));
    };

    if let Some(rec) = existing {
        let mut active: tenant_smtp_config::ActiveModel = rec.into();
        active.host = Set(req.host);
        active.port = Set(port);
        active.username = Set(req.username);
        active.password_enc = Set(password_enc);
        active.from_name = Set(req.from_name);
        active.from_email = Set(req.from_email);
        active.use_tls = Set(use_tls);
        active.enabled = Set(enabled);
        active.updated_at = Set(now);
        active.update(&state.db).await?;
    } else {
        tenant_smtp_config::ActiveModel {
            tenant_id: Set(tenant_id),
            host: Set(req.host),
            port: Set(port),
            username: Set(req.username),
            password_enc: Set(password_enc),
            from_name: Set(req.from_name),
            from_email: Set(req.from_email),
            use_tls: Set(use_tls),
            enabled: Set(enabled),
            updated_at: Set(now),
        }
        .insert(&state.db)
        .await?;
    }

    Ok(Json(serde_json::json!({ "message": "SMTP config saved" })))
}
