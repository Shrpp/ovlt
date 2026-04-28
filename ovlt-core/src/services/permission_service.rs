use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::{
    entity::{permissions, role_permissions, user_roles},
    error::AppError,
};

pub struct CreatePermissionInput {
    pub tenant_id: Uuid,
    pub name: String,
    pub description: String,
}

pub async fn create(
    txn: &DatabaseTransaction,
    input: CreatePermissionInput,
) -> Result<permissions::Model, AppError> {
    Ok(permissions::ActiveModel {
        tenant_id: Set(input.tenant_id),
        name: Set(input.name),
        description: Set(input.description),
        ..Default::default()
    }
    .insert(txn)
    .await?)
}

pub async fn list_all(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> Result<Vec<permissions::Model>, AppError> {
    Ok(permissions::Entity::find()
        .filter(permissions::Column::TenantId.eq(tenant_id))
        .all(txn)
        .await?)
}

pub async fn find_by_id(
    txn: &DatabaseTransaction,
    id: Uuid,
) -> Result<Option<permissions::Model>, AppError> {
    Ok(permissions::Entity::find_by_id(id).one(txn).await?)
}

pub async fn update(
    txn: &DatabaseTransaction,
    id: Uuid,
    name: String,
    description: String,
) -> Result<(), AppError> {
    let perm = find_by_id(txn, id).await?.ok_or(AppError::NotFound)?;
    let mut active: permissions::ActiveModel = perm.into();
    active.name = Set(name);
    active.description = Set(description);
    active.update(txn).await?;
    Ok(())
}

pub async fn delete(txn: &DatabaseTransaction, id: Uuid) -> Result<(), AppError> {
    permissions::Entity::delete_by_id(id).exec(txn).await?;
    Ok(())
}

// ── Role ↔ Permission ─────────────────────────────────────────────────────────

pub async fn list_for_role(
    txn: &DatabaseTransaction,
    role_id: Uuid,
    tenant_id: Uuid,
) -> Result<Vec<permissions::Model>, AppError> {
    let perm_ids: Vec<Uuid> = role_permissions::Entity::find()
        .filter(role_permissions::Column::RoleId.eq(role_id))
        .filter(role_permissions::Column::TenantId.eq(tenant_id))
        .all(txn)
        .await?
        .into_iter()
        .map(|rp| rp.permission_id)
        .collect();

    if perm_ids.is_empty() {
        return Ok(vec![]);
    }

    Ok(permissions::Entity::find()
        .filter(permissions::Column::Id.is_in(perm_ids))
        .filter(permissions::Column::TenantId.eq(tenant_id))
        .all(txn)
        .await?)
}

pub async fn assign_to_role(
    txn: &DatabaseTransaction,
    role_id: Uuid,
    permission_id: Uuid,
    tenant_id: Uuid,
) -> Result<(), AppError> {
    let now = Utc::now().fixed_offset();
    role_permissions::ActiveModel {
        role_id: Set(role_id),
        permission_id: Set(permission_id),
        tenant_id: Set(tenant_id),
        assigned_at: Set(now),
    }
    .insert(txn)
    .await
    .ok();
    Ok(())
}

pub async fn revoke_from_role(
    txn: &DatabaseTransaction,
    role_id: Uuid,
    permission_id: Uuid,
) -> Result<(), AppError> {
    role_permissions::Entity::delete_by_id((role_id, permission_id))
        .exec(txn)
        .await?;
    Ok(())
}

// ── User permission names (via roles) ─────────────────────────────────────────

pub async fn list_names_for_user(
    txn: &DatabaseTransaction,
    user_id: Uuid,
    tenant_id: Uuid,
) -> Result<Vec<String>, AppError> {
    let role_ids: Vec<Uuid> = user_roles::Entity::find()
        .filter(user_roles::Column::UserId.eq(user_id))
        .filter(user_roles::Column::TenantId.eq(tenant_id))
        .all(txn)
        .await?
        .into_iter()
        .map(|ur| ur.role_id)
        .collect();

    if role_ids.is_empty() {
        return Ok(vec![]);
    }

    let perm_ids: Vec<Uuid> = role_permissions::Entity::find()
        .filter(role_permissions::Column::RoleId.is_in(role_ids))
        .all(txn)
        .await?
        .into_iter()
        .map(|rp| rp.permission_id)
        .collect();

    if perm_ids.is_empty() {
        return Ok(vec![]);
    }

    let perms = permissions::Entity::find()
        .filter(permissions::Column::Id.is_in(perm_ids))
        .all(txn)
        .await?;

    let mut names: Vec<String> = perms.into_iter().map(|p| p.name).collect();
    names.sort();
    names.dedup();
    Ok(names)
}
