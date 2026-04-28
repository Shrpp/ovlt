use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, IntoActiveModel, QueryFilter,
    Set,
};
use uuid::Uuid;

use crate::{
    entity::{client_roles, roles, user_roles},
    error::AppError,
};

pub struct CreateRoleInput {
    pub tenant_id: Uuid,
    pub name: String,
    pub description: String,
}

pub async fn create(
    txn: &DatabaseTransaction,
    input: CreateRoleInput,
) -> Result<roles::Model, AppError> {
    Ok(roles::ActiveModel {
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
) -> Result<Vec<roles::Model>, AppError> {
    Ok(roles::Entity::find()
        .filter(roles::Column::TenantId.eq(tenant_id))
        .all(txn)
        .await?)
}

pub async fn find_by_id(
    txn: &DatabaseTransaction,
    id: Uuid,
) -> Result<Option<roles::Model>, AppError> {
    Ok(roles::Entity::find_by_id(id).one(txn).await?)
}

pub async fn update(
    txn: &DatabaseTransaction,
    id: Uuid,
    name: String,
    description: String,
) -> Result<(), AppError> {
    let role = find_by_id(txn, id).await?.ok_or(AppError::NotFound)?;
    let mut active = role.into_active_model();
    active.name = Set(name);
    active.description = Set(description);
    active.update(txn).await?;
    Ok(())
}

pub async fn delete(txn: &DatabaseTransaction, id: Uuid) -> Result<(), AppError> {
    roles::Entity::delete_by_id(id).exec(txn).await?;
    Ok(())
}

pub async fn list_for_user(
    txn: &DatabaseTransaction,
    user_id: Uuid,
    tenant_id: Uuid,
) -> Result<Vec<roles::Model>, AppError> {
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

    Ok(roles::Entity::find()
        .filter(roles::Column::Id.is_in(role_ids))
        .filter(roles::Column::TenantId.eq(tenant_id))
        .all(txn)
        .await?)
}

pub async fn list_names_for_user(
    txn: &DatabaseTransaction,
    user_id: Uuid,
    tenant_id: Uuid,
) -> Result<Vec<String>, AppError> {
    Ok(list_for_user(txn, user_id, tenant_id)
        .await?
        .into_iter()
        .map(|r| r.name)
        .collect())
}

pub async fn assign(
    txn: &DatabaseTransaction,
    user_id: Uuid,
    role_id: Uuid,
    tenant_id: Uuid,
) -> Result<(), AppError> {
    let now = Utc::now().fixed_offset();
    user_roles::ActiveModel {
        user_id: Set(user_id),
        role_id: Set(role_id),
        tenant_id: Set(tenant_id),
        assigned_at: Set(now),
    }
    .insert(txn)
    .await
    .ok(); // ignore conflict if already assigned
    Ok(())
}

pub async fn revoke(
    txn: &DatabaseTransaction,
    user_id: Uuid,
    role_id: Uuid,
) -> Result<(), AppError> {
    user_roles::Entity::delete_by_id((user_id, role_id))
        .exec(txn)
        .await?;
    Ok(())
}

// ── Client roles ──────────────────────────────────────────────────────────────

pub async fn list_for_client(
    txn: &DatabaseTransaction,
    oauth_client_id: Uuid,
) -> Result<Vec<roles::Model>, AppError> {
    let role_ids: Vec<Uuid> = client_roles::Entity::find()
        .filter(client_roles::Column::OauthClientId.eq(oauth_client_id))
        .all(txn)
        .await?
        .into_iter()
        .map(|cr| cr.role_id)
        .collect();

    if role_ids.is_empty() {
        return Ok(vec![]);
    }

    Ok(roles::Entity::find()
        .filter(roles::Column::Id.is_in(role_ids))
        .all(txn)
        .await?)
}

/// Returns role names for a user that are also scoped to the given OAuth client.
/// Used to populate `resource_access.<client_id>.roles` in tokens.
pub async fn list_client_role_names_for_user(
    txn: &DatabaseTransaction,
    user_id: Uuid,
    oauth_client_id: Uuid,
    tenant_id: Uuid,
) -> Result<Vec<String>, AppError> {
    let user_role_ids: std::collections::HashSet<Uuid> = user_roles::Entity::find()
        .filter(user_roles::Column::UserId.eq(user_id))
        .filter(user_roles::Column::TenantId.eq(tenant_id))
        .all(txn)
        .await?
        .into_iter()
        .map(|ur| ur.role_id)
        .collect();

    let client_role_ids: std::collections::HashSet<Uuid> = client_roles::Entity::find()
        .filter(client_roles::Column::OauthClientId.eq(oauth_client_id))
        .all(txn)
        .await?
        .into_iter()
        .map(|cr| cr.role_id)
        .collect();

    let intersection: Vec<Uuid> = user_role_ids.intersection(&client_role_ids).copied().collect();

    if intersection.is_empty() {
        return Ok(vec![]);
    }

    Ok(roles::Entity::find()
        .filter(roles::Column::Id.is_in(intersection))
        .all(txn)
        .await?
        .into_iter()
        .map(|r| r.name)
        .collect())
}

pub async fn assign_client_role(
    txn: &DatabaseTransaction,
    oauth_client_id: Uuid,
    role_id: Uuid,
    tenant_id: Uuid,
) -> Result<(), AppError> {
    let now = Utc::now().fixed_offset();
    client_roles::ActiveModel {
        oauth_client_id: Set(oauth_client_id),
        role_id: Set(role_id),
        tenant_id: Set(tenant_id),
        assigned_at: Set(now),
    }
    .insert(txn)
    .await
    .ok();
    Ok(())
}

pub async fn revoke_client_role(
    txn: &DatabaseTransaction,
    oauth_client_id: Uuid,
    role_id: Uuid,
) -> Result<(), AppError> {
    client_roles::Entity::delete_by_id((oauth_client_id, role_id))
        .exec(txn)
        .await?;
    Ok(())
}
