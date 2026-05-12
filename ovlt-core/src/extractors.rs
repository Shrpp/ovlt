use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use crate::{db, error::AppError, middleware::tenant::TenantContext, state::AppState};

/// Axum extractor that opens a tenant-scoped transaction with RLS active.
///
/// Handlers that declare `TenantDb` cannot accidentally use a raw connection
/// that bypasses row-level security. The transaction is automatically scoped to
/// the tenant from the `X-Tenant-ID` / `X-Tenant-Slug` header.
///
/// Usage:
/// ```rust
/// pub async fn handler(db: TenantDb) -> Result<impl IntoResponse, AppError> {
///     let TenantDb { txn, tenant_id, tenant_key } = db;
///     // ... use &txn for SeaORM queries ...
///     txn.commit().await?;
/// }
/// ```
pub struct TenantDb {
    pub txn: DatabaseTransaction,
    pub tenant_id: Uuid,
    /// Decrypted per-tenant encryption key.
    pub tenant_key: String,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for TenantDb
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let ctx = parts
            .extensions
            .get::<TenantContext>()
            .cloned()
            .ok_or(AppError::Unauthorized)?;
        let txn = db::begin_tenant_txn(&app_state.db, ctx.tenant_id).await?;
        Ok(Self {
            txn,
            tenant_id: ctx.tenant_id,
            tenant_key: ctx.tenant_key,
        })
    }
}
