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

use crate::{entity::tenants, error::AppError, handlers::admin_auth, services::seed_service, state::AppState};

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
    admin_auth::require_admin(&headers, &state.config.admin_key, &state.config.jwt_secret, state.master_tenant_id)?;

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

    // Seed SuperAdmin role + default:super_admin permission for every new tenant.
    seed_service::seed_tenant_defaults(&state.db, tenant.id).await?;

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

#[derive(Debug, Serialize)]
pub struct TenantSlugEntry {
    pub slug: String,
    pub name: String,
}

pub async fn list_tenant_slugs(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let rows = tenants::Entity::find().all(&state.db).await?;
    let resp: Vec<TenantSlugEntry> = rows
        .into_iter()
        .map(|t| TenantSlugEntry { slug: t.slug, name: t.name })
        .collect();
    Ok(Json(resp))
}

pub async fn list_tenants(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let scope = admin_auth::require_admin(&headers, &state.config.admin_key, &state.config.jwt_secret, state.master_tenant_id)?;

    let tenants = if let Some(tenant_id) = scope {
        // SuperAdmin: return only their own tenant
        tenants::Entity::find()
            .filter(tenants::Column::Id.eq(tenant_id))
            .all(&state.db)
            .await?
    } else {
        // Master / admin key: full list
        tenants::Entity::find().all(&state.db).await?
    };

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
