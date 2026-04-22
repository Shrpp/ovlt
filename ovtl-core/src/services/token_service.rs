use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, Set,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{entity::refresh_tokens, error::AppError};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub tid: String,
    pub email: String,
    pub iat: i64,
    pub exp: i64,
}

pub fn generate_access_token(
    user_id: Uuid,
    tenant_id: Uuid,
    email: &str,
    secret: &str,
    expiration_minutes: i64,
) -> Result<String, AppError> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        tid: tenant_id.to_string(),
        email: email.to_string(),
        iat: now,
        exp: now + expiration_minutes * 60,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::TokenError(e.to_string()))
}

pub fn validate_access_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|d| d.claims)
    .map_err(|e| AppError::TokenError(e.to_string()))
}

pub fn generate_refresh_token() -> String {
    Uuid::new_v4().to_string()
}

pub fn hash_refresh_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub async fn store_refresh_token(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    user_id: Uuid,
    token_hash: String,
    expiration_days: i64,
) -> Result<(), AppError> {
    let expires_at = (Utc::now() + chrono::Duration::days(expiration_days)).fixed_offset();
    refresh_tokens::ActiveModel {
        tenant_id: Set(tenant_id),
        user_id: Set(user_id),
        token_hash: Set(token_hash),
        expires_at: Set(expires_at),
        ..Default::default()
    }
    .insert(txn)
    .await?;
    Ok(())
}

/// Find a non-expired, non-revoked refresh token by its hash (within RLS context).
pub async fn find_valid_refresh_token(
    txn: &DatabaseTransaction,
    token_hash: &str,
) -> Result<Option<refresh_tokens::Model>, AppError> {
    let now = Utc::now().fixed_offset();
    Ok(refresh_tokens::Entity::find()
        .filter(refresh_tokens::Column::TokenHash.eq(token_hash))
        .filter(refresh_tokens::Column::RevokedAt.is_null())
        .filter(refresh_tokens::Column::ExpiresAt.gt(now))
        .one(txn)
        .await?)
}

/// Mark one token as revoked.
pub async fn revoke_token(
    txn: &DatabaseTransaction,
    record: refresh_tokens::Model,
) -> Result<(), AppError> {
    let now = Utc::now().fixed_offset();
    let mut active: refresh_tokens::ActiveModel = record.into();
    active.revoked_at = Set(Some(now));
    active.update(txn).await?;
    Ok(())
}

/// Revoke every active refresh token belonging to a user (within RLS context).
pub async fn revoke_all_user_tokens(
    txn: &DatabaseTransaction,
    user_id: Uuid,
) -> Result<(), AppError> {
    txn.execute_unprepared(&format!(
        "UPDATE refresh_tokens SET revoked_at = now() \
         WHERE user_id = '{user_id}' AND revoked_at IS NULL"
    ))
    .await?;
    Ok(())
}

/// Delete expired tokens across all tenants.
/// Safe to call without RLS context — pandora owns the tables (no FORCE ROW LEVEL SECURITY).
pub async fn cleanup_expired_tokens(db: &DatabaseConnection) -> Result<u64, AppError> {
    let now = Utc::now().fixed_offset();
    let result = refresh_tokens::Entity::delete_many()
        .filter(refresh_tokens::Column::ExpiresAt.lt(now))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}
