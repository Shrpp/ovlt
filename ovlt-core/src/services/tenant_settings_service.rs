use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use uuid::Uuid;

use crate::{entity::tenant_settings, error::AppError};

#[derive(Debug, Clone)]
pub struct TenantSettings {
    pub lockout_max_attempts: usize,
    pub lockout_window_minutes: i64,
    pub lockout_duration_minutes: i64,
    pub access_token_ttl_minutes: i64,
    pub refresh_token_ttl_days: i64,
    pub allow_public_registration: bool,
    pub require_email_verified: bool,
}

impl Default for TenantSettings {
    fn default() -> Self {
        Self {
            lockout_max_attempts: 5,
            lockout_window_minutes: 15,
            lockout_duration_minutes: 15,
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 30,
            allow_public_registration: true,
            require_email_verified: false,
        }
    }
}

pub async fn get(db: &DatabaseConnection, tenant_id: Uuid) -> Result<TenantSettings, AppError> {
    match tenant_settings::Entity::find_by_id(tenant_id)
        .one(db)
        .await?
    {
        Some(s) => Ok(TenantSettings {
            lockout_max_attempts: s.lockout_max_attempts as usize,
            lockout_window_minutes: s.lockout_window_minutes as i64,
            lockout_duration_minutes: s.lockout_duration_minutes as i64,
            access_token_ttl_minutes: s.access_token_ttl_minutes as i64,
            refresh_token_ttl_days: s.refresh_token_ttl_days as i64,
            allow_public_registration: s.allow_public_registration,
            require_email_verified: s.require_email_verified,
        }),
        None => Ok(TenantSettings::default()),
    }
}

pub async fn upsert(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    lockout_max_attempts: i32,
    lockout_window_minutes: i32,
    lockout_duration_minutes: i32,
    access_token_ttl_minutes: i32,
    refresh_token_ttl_days: i32,
    allow_public_registration: bool,
    require_email_verified: bool,
) -> Result<(), AppError> {
    let now = Utc::now().fixed_offset();
    let existing = tenant_settings::Entity::find_by_id(tenant_id)
        .one(db)
        .await?;
    if let Some(rec) = existing {
        let mut active: tenant_settings::ActiveModel = rec.into();
        active.lockout_max_attempts = Set(lockout_max_attempts);
        active.lockout_window_minutes = Set(lockout_window_minutes);
        active.lockout_duration_minutes = Set(lockout_duration_minutes);
        active.access_token_ttl_minutes = Set(access_token_ttl_minutes);
        active.refresh_token_ttl_days = Set(refresh_token_ttl_days);
        active.allow_public_registration = Set(allow_public_registration);
        active.require_email_verified = Set(require_email_verified);
        active.updated_at = Set(now);
        active.update(db).await?;
    } else {
        tenant_settings::ActiveModel {
            tenant_id: Set(tenant_id),
            lockout_max_attempts: Set(lockout_max_attempts),
            lockout_window_minutes: Set(lockout_window_minutes),
            lockout_duration_minutes: Set(lockout_duration_minutes),
            access_token_ttl_minutes: Set(access_token_ttl_minutes),
            refresh_token_ttl_days: Set(refresh_token_ttl_days),
            allow_public_registration: Set(allow_public_registration),
            require_email_verified: Set(require_email_verified),
            updated_at: Set(now),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}
