use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use crate::entity::tenants;
use crate::error::AppError;

pub struct TenantRecord {
    pub id: Uuid,
    pub encryption_key_encrypted: String,
}

/// Fetches a tenant by ID — no RLS needed, tenants table has no policy.
pub async fn find_active(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<TenantRecord, AppError> {
    let tenant = tenants::Entity::find_by_id(tenant_id)
        .one(db)
        .await?
        .ok_or(AppError::NotFound)?;

    if !tenant.is_active {
        return Err(AppError::Unauthorized);
    }

    Ok(TenantRecord {
        id: tenant.id,
        encryption_key_encrypted: tenant.encryption_key,
    })
}
