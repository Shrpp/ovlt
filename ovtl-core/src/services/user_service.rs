use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::{entity::users, error::AppError};

pub struct CreateUserInput {
    pub tenant_id: Uuid,
    pub email_encrypted: String,
    pub email_lookup: String,
    pub password_hash: String,
}

pub async fn create(
    txn: &DatabaseTransaction,
    input: CreateUserInput,
) -> Result<users::Model, AppError> {
    let model = users::ActiveModel {
        tenant_id: Set(input.tenant_id),
        email: Set(input.email_encrypted),
        email_lookup: Set(input.email_lookup),
        password_hash: Set(input.password_hash),
        ..Default::default()
    };
    Ok(model.insert(txn).await?)
}

pub async fn find_by_email_lookup(
    txn: &DatabaseTransaction,
    lookup: &str,
) -> Result<Option<users::Model>, AppError> {
    Ok(users::Entity::find()
        .filter(users::Column::EmailLookup.eq(lookup))
        .one(txn)
        .await?)
}

pub async fn email_lookup_exists(
    txn: &DatabaseTransaction,
    lookup: &str,
) -> Result<bool, AppError> {
    Ok(find_by_email_lookup(txn, lookup).await?.is_some())
}
