use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entity::tenants;
use crate::error::AppError;

pub struct TenantRecord {
    pub id: Uuid,
    pub encryption_key_encrypted: String,
}

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

pub async fn list_all_active(db: &DatabaseConnection) -> Result<Vec<TenantRecord>, AppError> {
    let tenants = tenants::Entity::find()
        .filter(tenants::Column::IsActive.eq(true))
        .all(db)
        .await?;
    Ok(tenants
        .into_iter()
        .map(|t| TenantRecord {
            id: t.id,
            encryption_key_encrypted: t.encryption_key,
        })
        .collect())
}

pub async fn find_active_by_slug(
    db: &DatabaseConnection,
    slug: &str,
) -> Result<TenantRecord, AppError> {
    let tenant = tenants::Entity::find()
        .filter(tenants::Column::Slug.eq(slug))
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
