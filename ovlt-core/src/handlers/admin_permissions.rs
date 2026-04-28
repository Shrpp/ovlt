use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{
    db, error::AppError, handlers::admin_auth, services::permission_service, state::AppState,
};

fn extract_tenant_id(headers: &HeaderMap) -> Result<Uuid, AppError> {
    headers
        .get("x-ovlt-tenant-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::InvalidInput("x-ovlt-tenant-id header required".into()))
}

fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    admin_auth::require_admin(
        headers,
        &state.config.admin_key,
        &state.config.jwt_secret,
        state.master_tenant_id,
    )
    .map(|_| ())
}

#[derive(Debug, Serialize)]
pub struct PermissionResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePermissionRequest {
    #[validate(length(min = 1, max = 64))]
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePermissionRequest {
    #[validate(length(min = 1, max = 64))]
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssignPermissionRequest {
    pub permission_id: String,
}

// ── Permissions CRUD ──────────────────────────────────────────────────────────

pub async fn list_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let perms = permission_service::list_all(&txn, tenant_id).await?;
    txn.commit().await?;

    let resp: Vec<PermissionResponse> = perms
        .into_iter()
        .map(|p| PermissionResponse {
            id: p.id.to_string(),
            name: p.name,
            description: p.description,
            created_at: p.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(resp))
}

pub async fn create_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreatePermissionRequest>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let perm = permission_service::create(
        &txn,
        permission_service::CreatePermissionInput {
            tenant_id,
            name: payload.name,
            description: payload.description.unwrap_or_default(),
        },
    )
    .await?;
    txn.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(PermissionResponse {
            id: perm.id.to_string(),
            name: perm.name,
            description: perm.description,
            created_at: perm.created_at.to_rfc3339(),
        }),
    ))
}

pub async fn update_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdatePermissionRequest>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    permission_service::update(
        &txn,
        id,
        payload.name,
        payload.description.unwrap_or_default(),
    )
    .await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    permission_service::delete(&txn, id).await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Role ↔ Permission ─────────────────────────────────────────────────────────

pub async fn list_role_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let perms = permission_service::list_for_role(&txn, role_id, tenant_id).await?;
    txn.commit().await?;

    let resp: Vec<PermissionResponse> = perms
        .into_iter()
        .map(|p| PermissionResponse {
            id: p.id.to_string(),
            name: p.name,
            description: p.description,
            created_at: p.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(resp))
}

pub async fn assign_role_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role_id): Path<Uuid>,
    Json(payload): Json<AssignPermissionRequest>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let permission_id = Uuid::parse_str(&payload.permission_id)
        .map_err(|_| AppError::InvalidInput("invalid permission_id".into()))?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    permission_service::assign_to_role(&txn, role_id, permission_id, tenant_id).await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn revoke_role_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((role_id, permission_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    permission_service::revoke_from_role(&txn, role_id, permission_id).await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}
