use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, PaginatorTrait, Set};
use uuid::Uuid;

use crate::{config::Config, db, entity::tenants, error::AppError, services::user_service};

pub async fn run(db: &DatabaseConnection, config: &Config) -> Result<(), AppError> {
    let (Some(email), Some(password)) = (
        &config.bootstrap_admin_email,
        &config.bootstrap_admin_password,
    ) else {
        return Ok(());
    };

    let count = tenants::Entity::find().count(db).await?;
    if count > 0 {
        return Ok(());
    }

    let slug = config
        .bootstrap_tenant_slug
        .as_deref()
        .unwrap_or("master");

    let tenant_key_plain = format!(
        "{}{}",
        hex::encode(Uuid::new_v4().as_bytes()),
        hex::encode(Uuid::new_v4().as_bytes())
    );
    let encrypted_key = hefesto::encrypt(
        &tenant_key_plain,
        &config.tenant_wrap_key,
        &config.master_encryption_key,
    )?;

    let tenant_id = Uuid::new_v4();
    tenants::ActiveModel {
        id: Set(tenant_id),
        name: Set("Master".to_string()),
        slug: Set(slug.to_string()),
        encryption_key: Set(encrypted_key),
        ..Default::default()
    }
    .insert(db)
    .await?;

    let email_normalized = email.trim().to_lowercase();
    let email_lookup = hefesto::hash_for_lookup(&email_normalized, &tenant_key_plain);
    let email_encrypted = hefesto::encrypt(
        &email_normalized,
        &tenant_key_plain,
        &config.master_encryption_key,
    )?;
    let password_hash = hefesto::hash_password(password)?;

    let txn = db::begin_tenant_txn(db, tenant_id).await?;
    user_service::create(
        &txn,
        user_service::CreateUserInput {
            tenant_id,
            email_encrypted,
            email_lookup,
            password_hash,
        },
    )
    .await?;
    txn.commit().await?;

    tracing::info!(
        tenant_id = %tenant_id,
        slug,
        email = %email_normalized,
        "bootstrap: master tenant and admin user created"
    );

    Ok(())
}
