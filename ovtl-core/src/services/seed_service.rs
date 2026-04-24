use uuid::Uuid;

use crate::{
    db,
    error::AppError,
    services::{permission_service, role_service, user_service},
};
use sea_orm::DatabaseConnection;

const SUPER_ADMIN_ROLE: &str = "SuperAdmin";
const SUPER_ADMIN_PERM: &str = "default:super_admin";
const SUPER_ADMIN_PERM_DESC: &str =
    "Grants full administrative access to this tenant";
const SUPER_ADMIN_ROLE_DESC: &str =
    "Full administrative access — can manage users, roles, clients, and settings";

/// Idempotent: creates the SuperAdmin role and default:super_admin permission
/// for a tenant, and links them. Safe to call multiple times.
pub async fn seed_tenant_defaults(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<(), AppError> {
    let txn = db::begin_tenant_txn(db, tenant_id).await?;

    // Ensure permission exists (unique constraint on tenant_id+name handles duplicates).
    let all_perms = permission_service::list_all(&txn, tenant_id).await?;
    let perm = if let Some(p) = all_perms.into_iter().find(|p| p.name == SUPER_ADMIN_PERM) {
        p
    } else {
        permission_service::create(
            &txn,
            permission_service::CreatePermissionInput {
                tenant_id,
                name: SUPER_ADMIN_PERM.to_string(),
                description: SUPER_ADMIN_PERM_DESC.to_string(),
            },
        )
        .await?
    };

    // Ensure role exists.
    let all_roles = role_service::list_all(&txn, tenant_id).await?;
    let role = if let Some(r) = all_roles.into_iter().find(|r| r.name == SUPER_ADMIN_ROLE) {
        r
    } else {
        role_service::create(
            &txn,
            role_service::CreateRoleInput {
                tenant_id,
                name: SUPER_ADMIN_ROLE.to_string(),
                description: SUPER_ADMIN_ROLE_DESC.to_string(),
            },
        )
        .await?
    };

    // Assign permission to role (assign_to_role ignores conflict).
    permission_service::assign_to_role(&txn, role.id, perm.id, tenant_id).await?;

    txn.commit().await?;
    Ok(())
}

/// Assign the SuperAdmin role to a user by email lookup hash.
/// Call this after creating the bootstrap admin user.
pub async fn assign_super_admin_role(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let txn = db::begin_tenant_txn(db, tenant_id).await?;

    let roles = role_service::list_all(&txn, tenant_id).await?;
    if let Some(role) = roles.into_iter().find(|r| r.name == SUPER_ADMIN_ROLE) {
        role_service::assign(&txn, user_id, role.id, tenant_id).await?;
    }

    txn.commit().await?;
    Ok(())
}
