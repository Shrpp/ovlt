use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;
use webauthn_rs::prelude::Passkey;

use crate::{entity::webauthn_credential, error::AppError};

pub async fn save_credential<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
    passkey: &Passkey,
    name: &str,
) -> Result<(), AppError> {
    let cred_id = base64_cred_id(passkey);
    let public_key_json =
        serde_json::to_string(passkey).map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let model = webauthn_credential::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        user_id: Set(user_id),
        credential_id: Set(cred_id),
        public_key_json: Set(public_key_json),
        name: Set(name.to_string()),
        aaguid: Set(None),
        sign_count: Set(0),
        created_at: Set(Utc::now().into()),
        last_used_at: Set(None),
    };
    model.insert(db).await?;
    Ok(())
}

pub async fn list_for_user<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<webauthn_credential::Model>, AppError> {
    Ok(webauthn_credential::Entity::find()
        .filter(webauthn_credential::Column::TenantId.eq(tenant_id))
        .filter(webauthn_credential::Column::UserId.eq(user_id))
        .all(db)
        .await?)
}

pub async fn list_passkeys_for_user<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<Passkey>, AppError> {
    let rows = list_for_user(db, tenant_id, user_id).await?;
    rows.iter()
        .map(|r| {
            serde_json::from_str::<Passkey>(&r.public_key_json)
                .map_err(|e| AppError::InvalidInput(e.to_string()))
        })
        .collect()
}

pub async fn update_after_auth<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    passkey: &Passkey,
) -> Result<(), AppError> {
    let cred_id = base64_cred_id(passkey);
    let row = webauthn_credential::Entity::find()
        .filter(webauthn_credential::Column::TenantId.eq(tenant_id))
        .filter(webauthn_credential::Column::CredentialId.eq(&cred_id))
        .one(db)
        .await?
        .ok_or(AppError::NotFound)?;

    let public_key_json =
        serde_json::to_string(passkey).map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let mut active: webauthn_credential::ActiveModel = row.into();
    active.public_key_json = Set(public_key_json);
    active.last_used_at = Set(Some(Utc::now().into()));
    active.update(db).await?;
    Ok(())
}

pub async fn delete<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    credential_id: &str,
) -> Result<(), AppError> {
    let result = webauthn_credential::Entity::delete_many()
        .filter(webauthn_credential::Column::TenantId.eq(tenant_id))
        .filter(webauthn_credential::Column::CredentialId.eq(credential_id))
        .exec(db)
        .await?;
    if result.rows_affected == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub async fn delete_all_for_user<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    webauthn_credential::Entity::delete_many()
        .filter(webauthn_credential::Column::TenantId.eq(tenant_id))
        .filter(webauthn_credential::Column::UserId.eq(user_id))
        .exec(db)
        .await?;
    Ok(())
}

fn base64_cred_id(passkey: &Passkey) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.encode(passkey.cred_id())
}
