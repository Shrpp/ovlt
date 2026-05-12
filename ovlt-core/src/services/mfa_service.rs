use base32::Alphabet;
use hex;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use totp_lite::{totp_custom, DEFAULT_STEP};
use uuid::Uuid;

use crate::{
    entity::{mfa_backup_codes, totp_secrets},
    error::AppError,
};

pub const BACKUP_CODE_COUNT: usize = 10;

fn backup_code_plaintext() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 5];
    rand::thread_rng().fill_bytes(&mut bytes);
    let encoded = base32::encode(Alphabet::RFC4648 { padding: false }, &bytes);
    format!("{}-{}", &encoded[..4], &encoded[4..])
}

fn hash_backup_code(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.to_uppercase().replace('-', "").as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn generate_backup_codes<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<String>, AppError> {
    mfa_backup_codes::Entity::delete_many()
        .filter(mfa_backup_codes::Column::TenantId.eq(tenant_id))
        .filter(mfa_backup_codes::Column::UserId.eq(user_id))
        .exec(db)
        .await?;

    let mut codes = Vec::with_capacity(BACKUP_CODE_COUNT);
    for _ in 0..BACKUP_CODE_COUNT {
        let code = backup_code_plaintext();
        let code_hash = hash_backup_code(&code);
        mfa_backup_codes::ActiveModel {
            tenant_id: Set(tenant_id),
            user_id: Set(user_id),
            code_hash: Set(code_hash),
            ..Default::default()
        }
        .insert(db)
        .await?;
        codes.push(code);
    }

    Ok(codes)
}

pub async fn consume_backup_code<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
    code: &str,
) -> Result<bool, AppError> {
    let code_hash = hash_backup_code(code);

    let record = mfa_backup_codes::Entity::find()
        .filter(mfa_backup_codes::Column::TenantId.eq(tenant_id))
        .filter(mfa_backup_codes::Column::UserId.eq(user_id))
        .filter(mfa_backup_codes::Column::CodeHash.eq(&code_hash))
        .filter(mfa_backup_codes::Column::UsedAt.is_null())
        .one(db)
        .await?;

    let Some(record) = record else {
        return Ok(false);
    };

    let mut active: mfa_backup_codes::ActiveModel = record.into();
    active.used_at = Set(Some(chrono::Utc::now().fixed_offset()));
    active.update(db).await?;

    Ok(true)
}

pub async fn delete_backup_codes<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    mfa_backup_codes::Entity::delete_many()
        .filter(mfa_backup_codes::Column::TenantId.eq(tenant_id))
        .filter(mfa_backup_codes::Column::UserId.eq(user_id))
        .exec(db)
        .await?;
    Ok(())
}

pub fn generate_secret() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 20];
    rand::thread_rng().fill_bytes(&mut bytes);
    base32::encode(Alphabet::RFC4648 { padding: false }, &bytes)
}

pub fn totp_uri(secret: &str, email: &str, issuer: &str) -> String {
    format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm=SHA1&digits=6&period=30",
        urlencoding(issuer),
        urlencoding(email),
        secret,
        urlencoding(issuer),
    )
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                vec![c]
            }
            c => format!("%{:02X}", c as u32).chars().collect(),
        })
        .collect()
}

pub fn verify_code(secret_b32: &str, code: &str) -> bool {
    let Some(secret_bytes) = base32::decode(Alphabet::RFC4648 { padding: false }, secret_b32)
    else {
        return false;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Allow one window drift in each direction
    for delta in [-1i64, 0, 1] {
        let t = (now as i64 + delta * DEFAULT_STEP as i64) as u64;
        let expected = totp_custom::<Sha1>(DEFAULT_STEP, 6, &secret_bytes, t);
        if expected == code {
            return true;
        }
    }
    false
}

pub async fn upsert_pending<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
    secret_enc: String,
) -> Result<(), AppError> {
    // Delete existing (whether enabled or not) before inserting fresh pending
    totp_secrets::Entity::delete_many()
        .filter(totp_secrets::Column::TenantId.eq(tenant_id))
        .filter(totp_secrets::Column::UserId.eq(user_id))
        .exec(db)
        .await?;

    totp_secrets::ActiveModel {
        tenant_id: Set(tenant_id),
        user_id: Set(user_id),
        secret_enc: Set(secret_enc),
        enabled: Set(false),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

pub async fn activate<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let record = totp_secrets::Entity::find()
        .filter(totp_secrets::Column::TenantId.eq(tenant_id))
        .filter(totp_secrets::Column::UserId.eq(user_id))
        .one(db)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut active: totp_secrets::ActiveModel = record.into();
    active.enabled = Set(true);
    active.update(db).await?;
    Ok(())
}

pub async fn disable<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    totp_secrets::Entity::delete_many()
        .filter(totp_secrets::Column::TenantId.eq(tenant_id))
        .filter(totp_secrets::Column::UserId.eq(user_id))
        .exec(db)
        .await?;
    Ok(())
}

pub async fn find_enabled<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Option<totp_secrets::Model>, AppError> {
    Ok(totp_secrets::Entity::find()
        .filter(totp_secrets::Column::TenantId.eq(tenant_id))
        .filter(totp_secrets::Column::UserId.eq(user_id))
        .filter(totp_secrets::Column::Enabled.eq(true))
        .one(db)
        .await?)
}

pub async fn find_any<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Option<totp_secrets::Model>, AppError> {
    Ok(totp_secrets::Entity::find()
        .filter(totp_secrets::Column::TenantId.eq(tenant_id))
        .filter(totp_secrets::Column::UserId.eq(user_id))
        .one(db)
        .await?)
}

pub async fn is_mfa_enabled_for_user<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<bool, AppError> {
    Ok(find_enabled(db, tenant_id, user_id).await?.is_some())
}
