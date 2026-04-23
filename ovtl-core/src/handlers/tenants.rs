use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{entity::tenants, error::AppError, state::AppState};

#[derive(Debug, Deserialize, Validate)]
pub struct CreateTenantRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(length(min = 1, max = 63))]
    pub slug: String,
}

#[derive(Debug, Serialize)]
pub struct TenantResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub plan: String,
    pub created_at: String,
}

fn require_admin_key(headers: &HeaderMap, expected: &Option<String>) -> Result<(), AppError> {
    let Some(key) = expected else {
        return Err(AppError::NotFound);
    };
    let provided = headers
        .get("x-ovtl-admin-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if provided != key {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

fn validate_slug(slug: &str) -> Result<(), AppError> {
    if slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
    {
        Ok(())
    } else {
        Err(AppError::InvalidInput(
            "slug must match [a-z0-9-] and not start or end with a dash".into(),
        ))
    }
}

pub async fn create_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTenantRequest>,
) -> Result<impl IntoResponse, AppError> {
    require_admin_key(&headers, &state.config.admin_key)?;

    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;
    validate_slug(&payload.slug)?;

    let slug_exists = tenants::Entity::find()
        .filter(tenants::Column::Slug.eq(&payload.slug))
        .one(&state.db)
        .await?
        .is_some();
    if slug_exists {
        return Err(AppError::Conflict);
    }

    let tenant_key_plain = format!(
        "{}{}",
        hex::encode(Uuid::new_v4().as_bytes()),
        hex::encode(Uuid::new_v4().as_bytes())
    );
    let encrypted_key = hefesto::encrypt(
        &tenant_key_plain,
        &state.config.tenant_wrap_key,
        &state.config.master_encryption_key,
    )?;

    let tenant = tenants::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(payload.name),
        slug: Set(payload.slug),
        encryption_key: Set(encrypted_key),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(TenantResponse {
            id: tenant.id.to_string(),
            name: tenant.name,
            slug: tenant.slug,
            plan: tenant.plan,
            created_at: tenant.created_at.to_rfc3339(),
        }),
    ))
}

pub async fn list_tenants(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    require_admin_key(&headers, &state.config.admin_key)?;

    let tenants = tenants::Entity::find().all(&state.db).await?;
    let response: Vec<TenantResponse> = tenants
        .into_iter()
        .map(|t| TenantResponse {
            id: t.id.to_string(),
            name: t.name,
            slug: t.slug,
            plan: t.plan,
            created_at: t.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(response))
}
