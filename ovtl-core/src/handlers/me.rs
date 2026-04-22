use axum::{extract::State, response::IntoResponse, Extension, Json};
use sea_orm::EntityTrait;
use serde_json::json;

use crate::{
    db,
    entity::users,
    error::AppError,
    middleware::{auth::AuthUser, tenant::TenantContext},
    state::AppState,
};

pub async fn me(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, AppError> {
    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let user = users::Entity::find_by_id(auth.user_id)
        .one(&txn)
        .await?
        .ok_or(AppError::NotFound)?;

    txn.commit().await?;

    let email = hefesto::decrypt(
        &user.email,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;

    Ok(Json(json!({
        "id": user.id,
        "email": email,
        "created_at": user.created_at.to_rfc3339(),
    })))
}
