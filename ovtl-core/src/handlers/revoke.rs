use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension};

use crate::{
    db,
    error::AppError,
    middleware::{auth::AuthUser, tenant::TenantContext},
    services::token_service,
    state::AppState,
};

pub async fn revoke(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, AppError> {
    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;
    token_service::revoke_all_user_tokens(&txn, auth.user_id).await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}
