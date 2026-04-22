use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use serde::Deserialize;

use crate::{
    db,
    error::AppError,
    middleware::{auth::AuthUser, tenant::TenantContext},
    services::token_service,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

pub async fn logout(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<LogoutRequest>,
) -> Result<impl IntoResponse, AppError> {
    let token_hash = token_service::hash_refresh_token(&payload.refresh_token);

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let record = token_service::find_valid_refresh_token(&txn, &token_hash).await?;

    // Only revoke if the token belongs to the authenticated user
    if let Some(r) = record {
        if r.user_id == auth.user_id {
            token_service::revoke_token(&txn, r).await?;
        }
    }

    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}
