use axum::{extract::State, response::IntoResponse, Extension, Json};
use sea_orm::EntityTrait;
use serde_json::json;

use crate::{
    entity::users, error::AppError, extractors::TenantDb, middleware::auth::AuthUser,
    state::AppState,
};

#[utoipa::path(
    get,
    path = "/users/me",
    tag = "user",
    responses(
        (status = 200, description = "Current user profile"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    params(
        ("X-Tenant-ID" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn me(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    db: TenantDb,
) -> Result<impl IntoResponse, AppError> {
    let TenantDb {
        txn, tenant_key, ..
    } = db;

    let user = users::Entity::find_by_id(auth.user_id)
        .one(&txn)
        .await?
        .ok_or(AppError::NotFound)?;

    txn.commit().await?;

    let email = hefesto::decrypt(
        &user.email,
        &tenant_key,
        &state.config.master_encryption_key,
    )?;

    Ok(Json(json!({
        "id": user.id,
        "email": email,
        "created_at": user.created_at.to_rfc3339(),
    })))
}
