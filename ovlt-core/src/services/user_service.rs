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

pub async fn find_by_id(
    txn: &DatabaseTransaction,
    id: Uuid,
) -> Result<Option<users::Model>, AppError> {
    Ok(users::Entity::find_by_id(id).one(txn).await?)
}

pub async fn email_lookup_exists(
    txn: &DatabaseTransaction,
    lookup: &str,
) -> Result<bool, AppError> {
    Ok(find_by_email_lookup(txn, lookup).await?.is_some())
}

pub async fn list_all(txn: &DatabaseTransaction) -> Result<Vec<users::Model>, AppError> {
    Ok(users::Entity::find().all(txn).await?)
}

pub async fn deactivate(txn: &DatabaseTransaction, id: Uuid) -> Result<(), AppError> {
    set_active(txn, id, false).await
}

pub async fn set_active(
    txn: &DatabaseTransaction,
    id: Uuid,
    is_active: bool,
) -> Result<(), AppError> {
    let user = users::Entity::find_by_id(id)
        .one(txn)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut active: users::ActiveModel = user.into();
    active.is_active = Set(is_active);
    active.update(txn).await?;
    Ok(())
}

pub async fn update_email(
    txn: &DatabaseTransaction,
    id: Uuid,
    email_encrypted: String,
    email_lookup: String,
) -> Result<(), AppError> {
    let user = users::Entity::find_by_id(id)
        .one(txn)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut active: users::ActiveModel = user.into();
    active.email = Set(email_encrypted);
    active.email_lookup = Set(email_lookup);
    active.update(txn).await?;
    Ok(())
}

pub async fn update_password(
    txn: &DatabaseTransaction,
    id: Uuid,
    password_hash: String,
) -> Result<(), AppError> {
    let user = users::Entity::find_by_id(id)
        .one(txn)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut active: users::ActiveModel = user.into();
    active.password_hash = Set(password_hash);
    active.update(txn).await?;
    Ok(())
}
