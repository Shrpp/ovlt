use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use uuid::Uuid;

use crate::{entity::password_policies, error::AppError};

pub struct Policy {
    pub min_length: usize,
    pub require_uppercase: bool,
    pub require_digit: bool,
    pub require_special: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            min_length: 8,
            require_uppercase: false,
            require_digit: false,
            require_special: false,
        }
    }
}

pub async fn get(db: &DatabaseConnection, tenant_id: Uuid) -> Result<Policy, AppError> {
    match password_policies::Entity::find_by_id(tenant_id)
        .one(db)
        .await?
    {
        Some(p) => Ok(Policy {
            min_length: p.min_length as usize,
            require_uppercase: p.require_uppercase,
            require_digit: p.require_digit,
            require_special: p.require_special,
        }),
        None => Ok(Policy::default()),
    }
}

pub async fn upsert(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    min_length: i32,
    require_uppercase: bool,
    require_digit: bool,
    require_special: bool,
    history_size: i32,
) -> Result<(), AppError> {
    let existing = password_policies::Entity::find_by_id(tenant_id)
        .one(db)
        .await?;
    let now = Utc::now().fixed_offset();
    if let Some(rec) = existing {
        let mut active: password_policies::ActiveModel = rec.into();
        active.min_length = Set(min_length);
        active.require_uppercase = Set(require_uppercase);
        active.require_digit = Set(require_digit);
        active.require_special = Set(require_special);
        active.history_size = Set(history_size);
        active.updated_at = Set(now);
        active.update(db).await?;
    } else {
        password_policies::ActiveModel {
            tenant_id: Set(tenant_id),
            min_length: Set(min_length),
            require_uppercase: Set(require_uppercase),
            require_digit: Set(require_digit),
            require_special: Set(require_special),
            history_size: Set(history_size),
            updated_at: Set(now),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

pub fn validate(password: &str, policy: &Policy) -> Result<(), AppError> {
    if password.len() < policy.min_length {
        return Err(AppError::InvalidInput(format!(
            "password must be at least {} characters",
            policy.min_length
        )));
    }
    if policy.require_uppercase && !password.chars().any(|c| c.is_uppercase()) {
        return Err(AppError::InvalidInput(
            "password must contain at least one uppercase letter".into(),
        ));
    }
    if policy.require_digit && !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(AppError::InvalidInput(
            "password must contain at least one digit".into(),
        ));
    }
    const SPECIAL: &str = "!@#$%^&*()_+-=[]{}|;':\",./<>?";
    if policy.require_special && !password.chars().any(|c| SPECIAL.contains(c)) {
        return Err(AppError::InvalidInput(
            "password must contain at least one special character".into(),
        ));
    }
    Ok(())
}
