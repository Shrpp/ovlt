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
    db,
    error::AppError,
    handlers::admin_auth,
    services::role_service,
    state::AppState,
};

fn extract_tenant_id(headers: &HeaderMap) -> Result<Uuid, AppError> {
    headers
        .get("x-ovtl-tenant-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::InvalidInput("x-ovtl-tenant-id header required".into()))
}

fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    admin_auth::require_admin(
        headers,
        &state.config.admin_key,
        &state.config.jwt_secret,
        state.master_tenant_id,
    ).map(|_| ())
}

#[derive(Debug, Serialize)]
pub struct RoleResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRoleRequest {
    #[validate(length(min = 1, max = 64))]
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRoleRequest {
    #[validate(length(min = 1, max = 64))]
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssignRoleRequest {
    pub role_id: String,
}

// ── Roles ─────────────────────────────────────────────────────────────────────

pub async fn list_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let roles = role_service::list_all(&txn, tenant_id).await?;
    txn.commit().await?;

    let resp: Vec<RoleResponse> = roles
        .into_iter()
        .map(|r| RoleResponse {
            id: r.id.to_string(),
            name: r.name,
            description: r.description,
            created_at: r.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(resp))
}

pub async fn create_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let role = role_service::create(
        &txn,
        role_service::CreateRoleInput {
            tenant_id,
            name: payload.name,
            description: payload.description.unwrap_or_default(),
        },
    )
    .await?;
    txn.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(RoleResponse {
            id: role.id.to_string(),
            name: role.name,
            description: role.description,
            created_at: role.created_at.to_rfc3339(),
        }),
    ))
}

pub async fn update_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    role_service::update(&txn, id, payload.name, payload.description.unwrap_or_default()).await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    role_service::delete(&txn, id).await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

// ── User roles ────────────────────────────────────────────────────────────────

pub async fn list_user_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let roles = role_service::list_for_user(&txn, user_id, tenant_id).await?;
    txn.commit().await?;

    let resp: Vec<RoleResponse> = roles
        .into_iter()
        .map(|r| RoleResponse {
            id: r.id.to_string(),
            name: r.name,
            description: r.description,
            created_at: r.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(resp))
}

pub async fn assign_user_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<AssignRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let role_id = Uuid::parse_str(&payload.role_id)
        .map_err(|_| AppError::InvalidInput("invalid role_id".into()))?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    role_service::assign(&txn, user_id, role_id, tenant_id).await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn revoke_user_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, role_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    role_service::revoke(&txn, user_id, role_id).await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}
