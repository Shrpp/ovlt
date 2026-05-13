use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    db,
    error::AppError,
    handlers::admin_auth,
    services::{audit_service, tenant_service, webauthn_service},
    state::AppState,
};

fn extract_tenant_id(headers: &HeaderMap) -> Result<Uuid, AppError> {
    headers
        .get("x-ovlt-tenant-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::InvalidInput("x-ovlt-tenant-id header required".into()))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PasskeyInfo {
    pub credential_id: String,
    pub name: String,
    pub aaguid: Option<String>,
    pub sign_count: i32,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[utoipa::path(
    get,
    path = "/admin/users/{id}/passkeys",
    tag = "admin-webauthn",
    responses(
        (status = 200, description = "List of passkeys for user", body = Vec<PasskeyInfo>),
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
pub async fn list_passkeys(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    admin_auth::require_admin(
        &headers,
        &state.config,
        state.master_tenant_id,
    )?;
    let tenant_id = extract_tenant_id(&headers)?;
    let _ = tenant_service::find_active(&state.db, tenant_id).await?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let rows = webauthn_service::list_for_user(&txn, tenant_id, user_id).await?;
    txn.commit().await?;

    let response: Vec<PasskeyInfo> = rows
        .into_iter()
        .map(|r| PasskeyInfo {
            credential_id: r.credential_id,
            name: r.name,
            aaguid: r.aaguid,
            sign_count: r.sign_count,
            created_at: r.created_at.to_rfc3339(),
            last_used_at: r.last_used_at.map(|t| t.to_rfc3339()),
        })
        .collect();

    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/admin/users/{id}/passkeys/{cred_id}",
    tag = "admin-webauthn",
    responses(
        (status = 204, description = "Passkey deleted"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("id" = String, Path, description = "User UUID"),
        ("cred_id" = String, Path, description = "Credential ID"),
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn delete_passkey(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, credential_id)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, AppError> {
    let actor = admin_auth::extract_actor(&headers, &state.config);
    admin_auth::require_admin(
        &headers,
        &state.config,
        state.master_tenant_id,
    )?;
    let tenant_id = extract_tenant_id(&headers)?;
    let _ = tenant_service::find_active(&state.db, tenant_id).await?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    webauthn_service::delete(&txn, tenant_id, &credential_id).await?;
    txn.commit().await?;

    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(tenant_id, actor, "passkey.deleted", serde_json::json!({"user_id": user_id, "credential_id": credential_id.as_str()})),
    );

    Ok(axum::http::StatusCode::NO_CONTENT)
}
