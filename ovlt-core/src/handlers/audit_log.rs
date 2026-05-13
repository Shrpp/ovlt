use axum::{extract::{Query, State}, http::HeaderMap, response::IntoResponse, Json};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{entity::audit_log, error::AppError, handlers::admin_auth, state::AppState};

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub limit: Option<u64>,
}

fn extract_tenant_id(headers: &HeaderMap) -> Result<Uuid, AppError> {
    headers
        .get("x-ovlt-tenant-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::InvalidInput("x-ovlt-tenant-id header required".into()))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AuditLogEntry {
    pub id: String,
    pub user_id: Option<String>,
    pub action: String,
    pub ip: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
}

#[utoipa::path(
    get,
    path = "/audit-log",
    tag = "audit",
    responses(
        (status = 200, description = "List of audit log entries", body = Vec<AuditLogEntry>),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn list_audit_log(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<AuditLogQuery>,
) -> Result<impl IntoResponse, AppError> {
    admin_auth::require_admin(
        &headers,
        &state.config,
        state.master_tenant_id,
    )?;
    let tenant_id = extract_tenant_id(&headers)?;
    let limit = params.limit.unwrap_or(100).min(10_000);

    let entries = audit_log::Entity::find()
        .filter(audit_log::Column::TenantId.eq(tenant_id))
        .order_by_desc(audit_log::Column::CreatedAt)
        .limit(limit)
        .all(&state.db)
        .await?;

    let response: Vec<AuditLogEntry> = entries
        .into_iter()
        .map(|e| AuditLogEntry {
            id: e.id.to_string(),
            user_id: e.user_id.map(|u| u.to_string()),
            action: e.action,
            ip: e.ip,
            metadata: e.metadata,
            created_at: e.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(response))
}
