use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{error::AppError, handlers::admin_auth, services::session_service, state::AppState};

fn extract_tenant_id(headers: &HeaderMap) -> Result<Uuid, AppError> {
    headers
        .get("x-ovlt-tenant-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::InvalidInput("x-ovlt-tenant-id header required".into()))
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub id: String,
    pub user_id: String,
    pub email: String,
    pub ip: Option<String>,
    pub created_at: String,
    pub last_seen_at: String,
    pub expires_at: String,
}

pub async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    admin_auth::require_admin(
        &headers,
        &state.config.admin_key,
        &state.config.jwt_secret,
        state.master_tenant_id,
    )?;
    let tenant_id = extract_tenant_id(&headers)?;
    let sessions = session_service::list_by_tenant(&state.db, tenant_id).await?;

    let response: Vec<SessionResponse> = sessions
        .into_iter()
        .map(|s| {
            let email = s.data["email"].as_str().unwrap_or("").to_string();
            let ip = s.data["ip"].as_str().map(|s| s.to_string());
            SessionResponse {
                id: s.id,
                user_id: s.user_id.to_string(),
                email,
                ip,
                created_at: s.created_at.to_rfc3339(),
                last_seen_at: s.last_seen_at.to_rfc3339(),
                expires_at: s.expires_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(Json(response))
}

pub async fn delete_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    admin_auth::require_admin(
        &headers,
        &state.config.admin_key,
        &state.config.jwt_secret,
        state.master_tenant_id,
    )?;
    let tenant_id = extract_tenant_id(&headers)?;

    // Verify the session belongs to this tenant before deleting.
    if let Some(session) = session_service::find_valid(&state.db, &id).await? {
        if session.tenant_id == tenant_id {
            session_service::delete(&state.db, &id).await?;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
