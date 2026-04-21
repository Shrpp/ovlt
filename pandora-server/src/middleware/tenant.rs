use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{error::AppError, services::tenant_service, state::AppState};

const TENANT_HEADER: &str = "x-pandora-tenant-id";

/// Tenant context injected into every protected request.
/// Handlers extract it via `Extension(ctx): Extension<TenantContext>`.
#[derive(Clone, Debug)]
pub struct TenantContext {
    pub tenant_id: Uuid,
    /// Decrypted per-tenant encryption key (lives only in memory).
    pub tenant_key: String,
}

/// Axum middleware — validates tenant header, decrypts tenant key, injects TenantContext.
pub async fn tenant_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let tenant_id = req
        .headers()
        .get(TENANT_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(AppError::Unauthorized)?;

    let record = tenant_service::find_active(&state.db, tenant_id).await?;

    // Tenant key wrapped with master_key (both layers). Phase 10: dedicated TENANT_WRAP_KEY.
    let tenant_key = hefesto::decrypt(
        &record.encryption_key_encrypted,
        &state.config.master_encryption_key,
        &state.config.master_encryption_key,
    )?;

    req.extensions_mut().insert(TenantContext {
        tenant_id,
        tenant_key,
    });

    Ok(next.run(req).await)
}
