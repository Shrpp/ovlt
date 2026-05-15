use axum::http::HeaderMap;
use uuid::Uuid;

use crate::{config::Config, error::AppError, services::token_service};

/// Extract the acting user's UUID from a Bearer JWT in the Authorization header.
/// Returns `None` if no token is present or validation fails — callers record
/// `None` as the actor when the request comes from a script using only the admin key.
pub fn extract_actor(headers: &HeaderMap, config: &Config) -> Option<Uuid> {
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))?;
    let claims = token_service::validate_access_token(
        bearer,
        &config.jwt_secret,
        config.jwt_secret_previous.as_deref(),
    )
    .ok()?;
    Uuid::parse_str(&claims.sub).ok()
}

/// Accept any of:
/// 1. X-OVLT-Admin-Key header matching the configured key.
/// 2. Bearer JWT from the master tenant.
/// 3. Bearer JWT whose `realm_access.roles` contains "SuperAdmin" and whose
///    `tid` matches the `x-ovlt-tenant-id` header (tenant-scoped admin).
///
/// Returns `Ok(None)` for full access (paths 1 & 2) or `Ok(Some(tenant_id))`
/// for tenant-scoped access (path 3), enabling callers to filter results.
pub fn require_admin(
    headers: &HeaderMap,
    config: &Config,
    master_tenant_id: Option<Uuid>,
) -> Result<Option<Uuid>, AppError> {
    // 1. Static admin key — full access.
    if let Some(key) = &config.admin_key {
        let provided = headers
            .get("x-ovlt-admin-key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if provided == key {
            return Ok(None);
        }
    }

    let Some(bearer) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    else {
        return Err(AppError::Unauthorized);
    };

    let claims = token_service::validate_access_token(
        bearer,
        &config.jwt_secret,
        config.jwt_secret_previous.as_deref(),
    )
    .map_err(|_| AppError::Unauthorized)?;

    let token_tid = Uuid::parse_str(&claims.tid).map_err(|_| AppError::Unauthorized)?;

    // 2. Master tenant — full access.
    if let Some(master_id) = master_tenant_id {
        if token_tid == master_id {
            return Ok(None);
        }
    }

    // 3. SuperAdmin role — tenant-scoped access.
    //    If x-ovlt-tenant-id header is present it must match the JWT's tid (blocks cross-tenant
    //    access). If the header is absent (e.g. list_tenants) the call is still allowed and
    //    scoped to the JWT's tid via the returned Some(token_tid).
    if claims.realm_access.roles.iter().any(|r| r == "SuperAdmin") {
        let header_tid = headers
            .get("x-ovlt-tenant-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| Uuid::parse_str(s).ok());

        if header_tid.is_none_or(|h| h == token_tid) {
            return Ok(Some(token_tid));
        }
    }

    Err(AppError::Unauthorized)
}
