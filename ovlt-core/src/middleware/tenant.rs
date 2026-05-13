use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{
    error::AppError,
    services::tenant_service,
    state::{AppState, CachedTenantKey},
};

#[derive(Clone, Debug)]
pub struct TenantContext {
    pub tenant_id: Uuid,
    /// Decrypted per-tenant encryption key (lives only in memory for the duration of the request).
    pub tenant_key: String,
}

pub async fn tenant_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let headers = req.headers();

    let record = if let Some(id_val) = headers
        .get("x-ovlt-tenant-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        tenant_service::find_active(&state.db, id_val).await?
    } else if let Some(slug) = headers
        .get("x-ovlt-tenant-slug")
        .and_then(|v| v.to_str().ok())
    {
        tenant_service::find_active_by_slug(&state.db, slug).await?
    } else {
        return Err(AppError::Unauthorized);
    };

    let tenant_key = {
        let cached = state.tenant_key_cache.get(&record.id);
        match cached {
            Some(entry) if entry.is_valid() => entry.get().to_string(),
            _ => {
                drop(cached);
                state.tenant_key_cache.remove(&record.id);
                let key = hefesto::decrypt(
                    &record.encryption_key_encrypted,
                    &state.config.tenant_wrap_key,
                    &state.config.master_encryption_key,
                )?;
                state
                    .tenant_key_cache
                    .insert(record.id, CachedTenantKey::new(key.clone()));
                key
            }
        }
    };

    req.extensions_mut().insert(TenantContext {
        tenant_id: record.id,
        tenant_key,
    });

    Ok(next.run(req).await)
}
