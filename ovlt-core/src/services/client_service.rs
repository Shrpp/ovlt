use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{entity::oauth_clients, error::AppError};

pub struct CreateClientInput {
    pub tenant_id: Uuid,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub is_confidential: bool,
    pub access_token_ttl_minutes: Option<i32>,
    pub refresh_token_ttl_days: Option<i32>,
}

/// Returns `(model, plain_secret)`.
/// For confidential clients, `plain_secret` is `Some(secret)` — returned only once, caller must surface it.
/// For public clients, `plain_secret` is `None` and the stored secret is `""`.
pub async fn create(
    txn: &DatabaseTransaction,
    input: CreateClientInput,
) -> Result<(oauth_clients::Model, Option<String>), AppError> {
    let (secret_hash, plain_secret) = if input.is_confidential {
        let plain = hex::encode(Sha256::digest(Uuid::new_v4().as_bytes()))
            + &hex::encode(Sha256::digest(Uuid::new_v4().as_bytes()));
        let hash = hex::encode(Sha256::digest(plain.as_bytes()));
        (hash, Some(plain))
    } else {
        (String::new(), None)
    };

    let model = oauth_clients::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(input.tenant_id),
        client_id: Set(Uuid::new_v4().to_string()),
        client_secret: Set(secret_hash),
        name: Set(input.name),
        redirect_uris: Set(serde_json::json!(input.redirect_uris)),
        grant_types: Set(serde_json::json!(input.grant_types)),
        scopes: Set(serde_json::json!(input.scopes)),
        is_confidential: Set(input.is_confidential),
        require_consent: Set(false),
        is_active: Set(true),
        access_token_ttl_minutes: Set(input.access_token_ttl_minutes),
        refresh_token_ttl_days: Set(input.refresh_token_ttl_days),
        ..Default::default()
    }
    .insert(txn)
    .await?;

    Ok((model, plain_secret))
}

/// Find a client by client_id across all tenants.
/// Uses `DatabaseConnection` (superuser, bypasses RLS) — for client_credentials flow.
pub async fn find_by_client_id_global(
    db: &DatabaseConnection,
    client_id: &str,
) -> Result<Option<oauth_clients::Model>, AppError> {
    Ok(oauth_clients::Entity::find()
        .filter(oauth_clients::Column::ClientId.eq(client_id))
        .filter(oauth_clients::Column::IsActive.eq(true))
        .one(db)
        .await?)
}

pub async fn find_by_client_id(
    txn: &DatabaseTransaction,
    client_id: &str,
) -> Result<Option<oauth_clients::Model>, AppError> {
    Ok(oauth_clients::Entity::find()
        .filter(oauth_clients::Column::ClientId.eq(client_id))
        .filter(oauth_clients::Column::IsActive.eq(true))
        .one(txn)
        .await?)
}

pub async fn find_by_id(
    txn: &DatabaseTransaction,
    id: Uuid,
) -> Result<Option<oauth_clients::Model>, AppError> {
    Ok(oauth_clients::Entity::find_by_id(id).one(txn).await?)
}

pub async fn list(txn: &DatabaseTransaction) -> Result<Vec<oauth_clients::Model>, AppError> {
    Ok(oauth_clients::Entity::find().all(txn).await?)
}

pub struct UpdateClientInput {
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub access_token_ttl_minutes: Option<i32>,
    pub refresh_token_ttl_days: Option<i32>,
    pub is_confidential: bool,
    pub grant_types: Vec<String>,
}

pub async fn update(
    txn: &DatabaseTransaction,
    id: Uuid,
    input: UpdateClientInput,
) -> Result<oauth_clients::Model, AppError> {
    let model = oauth_clients::Entity::find_by_id(id)
        .one(txn)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut active: oauth_clients::ActiveModel = model.into();
    active.name = Set(input.name);
    active.redirect_uris = Set(serde_json::json!(input.redirect_uris));
    active.scopes = Set(serde_json::json!(input.scopes));
    active.access_token_ttl_minutes = Set(input.access_token_ttl_minutes);
    active.refresh_token_ttl_days = Set(input.refresh_token_ttl_days);
    active.is_confidential = Set(input.is_confidential);
    active.grant_types = Set(serde_json::json!(input.grant_types));
    Ok(active.update(txn).await?)
}

pub async fn deactivate(txn: &DatabaseTransaction, id: Uuid) -> Result<(), AppError> {
    let model = oauth_clients::Entity::find_by_id(id)
        .one(txn)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut active: oauth_clients::ActiveModel = model.into();
    active.is_active = Set(false);
    active.update(txn).await?;
    Ok(())
}

pub fn verify_secret(plain: &str, hash: &str) -> bool {
    hex::encode(Sha256::digest(plain.as_bytes())) == hash
}

pub fn redirect_uri_allowed(client: &oauth_clients::Model, uri: &str) -> bool {
    client
        .redirect_uris
        .as_array()
        .map(|arr| arr.iter().any(|v| v.as_str() == Some(uri)))
        .unwrap_or(false)
}

pub fn scopes_to_vec(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect()
        })
        .unwrap_or_default()
}
