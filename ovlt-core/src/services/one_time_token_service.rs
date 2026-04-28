use chrono::Utc;
use rand::{rngs::OsRng, RngCore};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{entity::one_time_tokens, error::AppError};

/// Long hex token for password reset links (64 chars, 256 bits).
pub fn generate() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Short 6-digit OTP for email verification shown to the admin.
/// Hashed as SHA256(user_id + ":" + otp) to avoid collisions across users.
pub fn generate_otp() -> String {
    let mut bytes = [0u8; 4];
    OsRng.fill_bytes(&mut bytes);
    let n = u32::from_be_bytes(bytes) % 1_000_000;
    format!("{n:06}")
}

pub fn hash_otp(user_id: Uuid, otp: &str) -> String {
    hex::encode(Sha256::digest(format!("{user_id}:{otp}").as_bytes()))
}

pub async fn store_otp(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    otp: &str,
    expires_in_hours: i64,
) -> Result<(), AppError> {
    let token_hash = hash_otp(user_id, otp);
    store(
        db,
        tenant_id,
        user_id,
        token_hash,
        one_time_tokens::TYPE_EMAIL_VERIFICATION,
        expires_in_hours * 60,
    )
    .await
}

pub async fn consume_otp(
    db: &DatabaseConnection,
    user_id: Uuid,
    otp: &str,
) -> Result<one_time_tokens::Model, AppError> {
    let hash = hash_otp(user_id, otp);
    let record = one_time_tokens::Entity::find()
        .filter(one_time_tokens::Column::TokenHash.eq(&hash))
        .filter(one_time_tokens::Column::TokenType.eq(one_time_tokens::TYPE_EMAIL_VERIFICATION))
        .filter(one_time_tokens::Column::UsedAt.is_null())
        .one(db)
        .await?
        .ok_or(AppError::InvalidInput("invalid or expired OTP".into()))?;

    let now = Utc::now().fixed_offset();
    if record.expires_at < now {
        return Err(AppError::InvalidInput("OTP expired".into()));
    }

    let mut active: one_time_tokens::ActiveModel = record.clone().into();
    active.used_at = Set(Some(now));
    active.update(db).await?;

    Ok(record)
}

pub async fn store(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    token_hash: String,
    token_type: &str,
    expires_in_minutes: i64,
) -> Result<(), AppError> {
    let expires_at = (Utc::now() + chrono::Duration::minutes(expires_in_minutes)).fixed_offset();
    one_time_tokens::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        user_id: Set(user_id),
        token_hash: Set(token_hash),
        token_type: Set(token_type.to_string()),
        expires_at: Set(expires_at),
        used_at: Set(None),
    }
    .insert(db)
    .await?;
    Ok(())
}

pub async fn consume(
    db: &DatabaseConnection,
    token: &str,
    expected_type: &str,
) -> Result<one_time_tokens::Model, AppError> {
    let hash = hash(token);
    let record = one_time_tokens::Entity::find()
        .filter(one_time_tokens::Column::TokenHash.eq(&hash))
        .filter(one_time_tokens::Column::TokenType.eq(expected_type))
        .one(db)
        .await?
        .ok_or(AppError::NotFound)?;

    if record.used_at.is_some() {
        return Err(AppError::InvalidInput("token already used".into()));
    }
    let now = Utc::now().fixed_offset();
    if record.expires_at < now {
        return Err(AppError::InvalidInput("token expired".into()));
    }

    let mut active: one_time_tokens::ActiveModel = record.clone().into();
    active.used_at = Set(Some(now));
    active.update(db).await?;

    Ok(record)
}

pub async fn cleanup_expired(db: &DatabaseConnection) -> Result<u64, AppError> {
    let cutoff = Utc::now().fixed_offset();
    let res = one_time_tokens::Entity::delete_many()
        .filter(one_time_tokens::Column::ExpiresAt.lt(cutoff))
        .filter(one_time_tokens::Column::UsedAt.is_not_null())
        .exec(db)
        .await?;
    Ok(res.rows_affected)
}
