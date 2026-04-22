use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Redirect},
    Extension, Json,
};
use serde::Deserialize;

use crate::{
    db,
    error::AppError,
    handlers::login::TokenResponse,
    middleware::tenant::TenantContext,
    services::{oauth_service, tenant_service, token_service},
    state::AppState,
};

/// `GET /auth/:provider` — redirects to provider's consent screen.
/// Requires X-Pandora-Tenant-ID header (tenant_middleware active).
pub async fn authorize(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Path(provider): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let (auth_url, _) =
        oauth_service::build_authorize_url(&provider, &state.config, ctx.tenant_id)?;
    Ok(Redirect::to(&auth_url))
}

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    pub code: String,
    pub state: String,
}

/// `GET /auth/:provider/callback` — no tenant header; tenant_id extracted from state.
pub async fn callback(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(params): Query<CallbackParams>,
) -> Result<impl IntoResponse, AppError> {
    // Verify CSRF state and recover tenant_id
    let tenant_id = oauth_service::verify_state(&params.state, &state.config.jwt_secret)
        .ok_or(AppError::Unauthorized)?;

    // Rebuild tenant context (no middleware here)
    let record = tenant_service::find_active(&state.db, tenant_id).await?;
    let tenant_key = hefesto::decrypt(
        &record.encryption_key_encrypted,
        &state.config.master_encryption_key,
        &state.config.master_encryption_key,
    )?;

    // Exchange authorization code for provider access token
    let provider_token = oauth_service::exchange_code(&provider, &params.code, &state.config).await?;

    // Fetch user profile from provider
    let profile = oauth_service::fetch_profile(&provider, &provider_token).await?;

    // Find or create local user + link oauth_account
    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let user = oauth_service::find_or_create_user(
        &txn,
        tenant_id,
        &tenant_key,
        &state.config.master_encryption_key,
        &provider,
        &profile,
    )
    .await?;

    // Issue Pandora tokens
    let email = hefesto::decrypt(&user.email, &tenant_key, &state.config.master_encryption_key)?;
    let access_token = token_service::generate_access_token(
        user.id,
        tenant_id,
        &email,
        &state.config.jwt_secret,
        state.config.jwt_expiration_minutes,
    )?;
    let refresh_token = token_service::generate_refresh_token();
    let token_hash = token_service::hash_refresh_token(&refresh_token);
    token_service::store_refresh_token(
        &txn,
        tenant_id,
        user.id,
        token_hash,
        state.config.refresh_token_expiration_days,
    )
    .await?;

    txn.commit().await?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token,
        expires_in: state.config.jwt_expiration_minutes * 60,
    }))
}
