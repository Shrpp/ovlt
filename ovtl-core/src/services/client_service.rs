use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{entity::oauth_clients, error::AppError};

pub struct CreateClientInput {
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub scopes: Vec<String>,
    pub is_confidential: bool,
    pub require_consent: bool,
}

pub fn hash_secret(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}

pub fn verify_secret(client: &oauth_clients::Model, plain: &str) -> bool {
    client.client_secret == hash_secret(plain)
}

pub async fn create(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    input: CreateClientInput,
) -> Result<(oauth_clients::Model, String), AppError> {
    let plain_secret = Uuid::new_v4().to_string();

    let model = oauth_clients::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        client_id: Set(Uuid::new_v4().to_string()),
        client_secret: Set(hash_secret(&plain_secret)),
        name: Set(input.name),
        redirect_uris: Set(serde_json::to_value(&input.redirect_uris).unwrap_or_default()),
        grant_types: Set(serde_json::to_value(&input.grant_types).unwrap_or_default()),
        scopes: Set(serde_json::to_value(&input.scopes).unwrap_or_default()),
        is_confidential: Set(input.is_confidential),
        require_consent: Set(input.require_consent),
        is_active: Set(true),
        ..Default::default()
    }
    .insert(txn)
    .await?;

    Ok((model, plain_secret))
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

pub async fn list(
    txn: &DatabaseTransaction,
) -> Result<Vec<oauth_clients::Model>, AppError> {
    Ok(oauth_clients::Entity::find().all(txn).await?)
}

pub async fn deactivate(
    txn: &DatabaseTransaction,
    id: Uuid,
) -> Result<(), AppError> {
    let client = oauth_clients::Entity::find_by_id(id)
        .one(txn)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut active: oauth_clients::ActiveModel = client.into();
    active.is_active = Set(false);
    active.update(txn).await?;
    Ok(())
}

pub fn redirect_uri_allowed(client: &oauth_clients::Model, uri: &str) -> bool {
    let uris: Vec<String> =
        serde_json::from_value(client.redirect_uris.clone()).unwrap_or_default();
    uris.iter().any(|u| u == uri)
}

pub fn scopes_allowed(client: &oauth_clients::Model, requested: &[&str]) -> bool {
    let allowed: Vec<String> =
        serde_json::from_value(client.scopes.clone()).unwrap_or_default();
    requested.iter().all(|s| allowed.contains(&s.to_string()))
}
