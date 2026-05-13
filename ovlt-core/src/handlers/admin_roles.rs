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
    error::{validation_to_app_error, AppError},
    handlers::admin_auth,
    services::{audit_service, role_service},
    state::AppState,
};

fn extract_tenant_id(headers: &HeaderMap) -> Result<Uuid, AppError> {
    headers
        .get("x-ovlt-tenant-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::InvalidInput("x-ovlt-tenant-id header required".into()))
}

fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    admin_auth::require_admin(headers, &state.config, state.master_tenant_id).map(|_| ())
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RoleResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateRoleRequest {
    #[validate(length(min = 1, max = 64))]
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateRoleRequest {
    #[validate(length(min = 1, max = 64))]
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AssignRoleRequest {
    pub role_id: String,
}

// ── Roles ─────────────────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/roles",
    tag = "admin-roles",
    responses(
        (status = 200, description = "List of roles", body = Vec<RoleResponse>),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
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

#[utoipa::path(
    post,
    path = "/roles",
    tag = "admin-roles",
    request_body = CreateRoleRequest,
    responses(
        (status = 201, description = "Role created", body = RoleResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn create_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let actor = admin_auth::extract_actor(&headers, &state.config);
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    payload.validate().map_err(validation_to_app_error)?;

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

    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(
            tenant_id,
            actor,
            "role.created",
            serde_json::json!({"role_id": role.id, "name": role.name.as_str()}),
        ),
    );

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

#[utoipa::path(
    put,
    path = "/roles/{id}",
    tag = "admin-roles",
    request_body = UpdateRoleRequest,
    responses(
        (status = 204, description = "Role updated"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("id" = String, Path, description = "Role UUID"),
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn update_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let actor = admin_auth::extract_actor(&headers, &state.config);
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    payload.validate().map_err(validation_to_app_error)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    role_service::update(
        &txn,
        id,
        payload.name,
        payload.description.unwrap_or_default(),
    )
    .await?;
    txn.commit().await?;

    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(
            tenant_id,
            actor,
            "role.updated",
            serde_json::json!({"role_id": id}),
        ),
    );

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/roles/{id}",
    tag = "admin-roles",
    responses(
        (status = 204, description = "Role deleted"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("id" = String, Path, description = "Role UUID"),
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn delete_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let actor = admin_auth::extract_actor(&headers, &state.config);
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    role_service::delete(&txn, id).await?;
    txn.commit().await?;

    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(
            tenant_id,
            actor,
            "role.deleted",
            serde_json::json!({"role_id": id}),
        ),
    );

    Ok(StatusCode::NO_CONTENT)
}

// ── User roles ────────────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/users/{id}/roles",
    tag = "admin-roles",
    responses(
        (status = 200, description = "List of roles for user", body = Vec<RoleResponse>),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("id" = String, Path, description = "User UUID"),
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
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

#[utoipa::path(
    post,
    path = "/users/{id}/roles",
    tag = "admin-roles",
    request_body = AssignRoleRequest,
    responses(
        (status = 204, description = "Role assigned to user"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("id" = String, Path, description = "User UUID"),
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn assign_user_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<AssignRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let actor = admin_auth::extract_actor(&headers, &state.config);
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let role_id = Uuid::parse_str(&payload.role_id)
        .map_err(|_| AppError::InvalidInput("invalid role_id".into()))?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    role_service::assign(&txn, user_id, role_id, tenant_id).await?;
    txn.commit().await?;

    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(
            tenant_id,
            actor,
            "user.role.assigned",
            serde_json::json!({"user_id": user_id, "role_id": role_id}),
        ),
    );

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/users/{user_id}/roles/{role_id}",
    tag = "admin-roles",
    responses(
        (status = 204, description = "Role revoked from user"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("user_id" = String, Path, description = "User UUID"),
        ("role_id" = String, Path, description = "Role UUID"),
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn revoke_user_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, role_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let actor = admin_auth::extract_actor(&headers, &state.config);
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    role_service::revoke(&txn, user_id, role_id).await?;
    txn.commit().await?;

    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(
            tenant_id,
            actor,
            "user.role.revoked",
            serde_json::json!({"user_id": user_id, "role_id": role_id}),
        ),
    );

    Ok(StatusCode::NO_CONTENT)
}

// ── Client roles ──────────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/clients/{id}/roles",
    tag = "admin-roles",
    responses(
        (status = 200, description = "List of roles for client", body = Vec<RoleResponse>),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("id" = String, Path, description = "Client UUID"),
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn list_client_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(client_uuid): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let roles = role_service::list_for_client(&txn, client_uuid).await?;
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

#[utoipa::path(
    post,
    path = "/clients/{id}/roles",
    tag = "admin-roles",
    request_body = AssignRoleRequest,
    responses(
        (status = 204, description = "Role assigned to client"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("id" = String, Path, description = "Client UUID"),
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn assign_client_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(client_uuid): Path<Uuid>,
    Json(payload): Json<AssignRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let actor = admin_auth::extract_actor(&headers, &state.config);
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let role_id = Uuid::parse_str(&payload.role_id)
        .map_err(|_| AppError::InvalidInput("invalid role_id".into()))?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    role_service::assign_client_role(&txn, client_uuid, role_id, tenant_id).await?;
    txn.commit().await?;

    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(
            tenant_id,
            actor,
            "client.role.assigned",
            serde_json::json!({"client_id": client_uuid, "role_id": role_id}),
        ),
    );

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/clients/{client_id}/roles/{role_id}",
    tag = "admin-roles",
    responses(
        (status = 204, description = "Role revoked from client"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("client_id" = String, Path, description = "Client UUID"),
        ("role_id" = String, Path, description = "Role UUID"),
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn revoke_client_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((client_uuid, role_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let actor = admin_auth::extract_actor(&headers, &state.config);
    require_admin(&state, &headers)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    role_service::revoke_client_role(&txn, client_uuid, role_id).await?;
    txn.commit().await?;

    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(
            tenant_id,
            actor,
            "client.role.revoked",
            serde_json::json!({"client_id": client_uuid, "role_id": role_id}),
        ),
    );

    Ok(StatusCode::NO_CONTENT)
}
