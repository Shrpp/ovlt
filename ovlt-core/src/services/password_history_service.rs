use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use uuid::Uuid;

use crate::{entity::password_history, error::AppError};

/// Verify `plaintext` does not match any of the last `history_size` stored
/// hashes for this user. No-ops when `history_size <= 0`.
pub async fn check<C: ConnectionTrait>(
    db: &C,
    user_id: Uuid,
    plaintext: &str,
    history_size: i32,
) -> Result<(), AppError> {
    if history_size <= 0 {
        return Ok(());
    }

    let entries = password_history::Entity::find()
        .filter(password_history::Column::UserId.eq(user_id))
        .order_by_desc(password_history::Column::CreatedAt)
        .limit(history_size as u64)
        .all(db)
        .await?;

    for entry in &entries {
        if hefesto::verify_password(plaintext, &entry.password_hash) {
            return Err(AppError::InvalidInput(format!(
                "cannot reuse any of your last {history_size} passwords"
            )));
        }
    }

    Ok(())
}

/// Store `password_hash` in history for the given user.
pub async fn record<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
    password_hash: &str,
) -> Result<(), AppError> {
    password_history::ActiveModel {
        tenant_id: Set(tenant_id),
        user_id: Set(user_id),
        password_hash: Set(password_hash.to_owned()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}
