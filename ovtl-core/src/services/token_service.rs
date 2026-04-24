use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, Set,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{entity::{refresh_tokens, revoked_jtis}, error::AppError};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RealmAccess {
    pub roles: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// JWT ID — unique per token, useful for correlation in audit logs.
    pub jti: String,
    pub sub: String,
    pub tid: String,
    pub email: String,
    pub iat: i64,
    pub exp: i64,
    #[serde(default)]
    pub realm_access: RealmAccess,
}

pub fn generate_access_token(
    user_id: Uuid,
    tenant_id: Uuid,
    email: &str,
    roles: Vec<String>,
    permissions: Vec<String>,
    secret: &str,
    expiration_minutes: i64,
) -> Result<String, AppError> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        jti: Uuid::new_v4().to_string(),
        sub: user_id.to_string(),
        tid: tenant_id.to_string(),
        email: email.to_string(),
        iat: now,
        exp: now + expiration_minutes * 60,
        realm_access: RealmAccess { roles, permissions },
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::TokenError(e.to_string()))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ClientTokenClaims {
    jti: String,
    sub: String,   // client_id string
    tid: String,
    iat: i64,
    exp: i64,
    scope: String, // space-joined scopes
}

pub fn generate_client_access_token(
    client_id: &str,
    tenant_id: Uuid,
    scopes: &[String],
    secret: &str,
    expiration_minutes: i64,
) -> Result<String, AppError> {
    let now = Utc::now().timestamp();
    let claims = ClientTokenClaims {
        jti: Uuid::new_v4().to_string(),
        sub: client_id.to_string(),
        tid: tenant_id.to_string(),
        iat: now,
        exp: now + expiration_minutes * 60,
        scope: scopes.join(" "),
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

pub async fn revoke_jti(
    db: &DatabaseConnection,
    jti: &str,
    expires_at: chrono::DateTime<chrono::FixedOffset>,
) -> Result<(), AppError> {
    revoked_jtis::ActiveModel {
        jti: Set(jti.to_owned()),
        expires_at: Set(expires_at),
    }
    .insert(db)
    .await
    .ok(); // ignore conflict if already revoked
    Ok(())
}

pub async fn is_jti_revoked(db: &DatabaseConnection, jti: &str) -> Result<bool, AppError> {
    Ok(revoked_jtis::Entity::find_by_id(jti)
        .one(db)
        .await?
        .is_some())
}

pub async fn cleanup_expired_jtis(db: &DatabaseConnection) -> Result<u64, AppError> {
    let now = Utc::now().fixed_offset();
    let result = revoked_jtis::Entity::delete_many()
        .filter(revoked_jtis::Column::ExpiresAt.lt(now))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}

/// Delete expired refresh tokens across all tenants.
/// refresh_tokens has no FORCE ROW LEVEL SECURITY, so the table owner can run this
/// without a tenant transaction context.
pub async fn cleanup_expired_tokens(db: &DatabaseConnection) -> Result<u64, AppError> {
    let now = Utc::now().fixed_offset();
    let result = refresh_tokens::Entity::delete_many()
        .filter(refresh_tokens::Column::ExpiresAt.lt(now))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}
