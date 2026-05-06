use axum::{extract::State, response::IntoResponse, Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use webauthn_rs::prelude::{PublicKeyCredential, RegisterPublicKeyCredential};

use crate::{
    db,
    error::AppError,
    middleware::{auth::AuthUser, tenant::TenantContext},
    services::{
        audit_service, permission_service, role_service, session_service, tenant_settings_service,
        token_service, user_service, webauthn_service,
    },
    state::AppState,
};

// ── Registration ─────────────────────────────────────────────────────────────

pub async fn register_start(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, AppError> {
    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let existing: Vec<_> =
        webauthn_service::list_passkeys_for_user(&txn, ctx.tenant_id, auth.user_id)
            .await?
            .into_iter()
            .map(|p| p.cred_id().clone())
            .collect();

    txn.commit().await?;

    let exclude = if existing.is_empty() {
        None
    } else {
        Some(existing)
    };

    let (ccr, reg_state) = state
        .webauthn
        .start_passkey_registration(auth.user_id, &auth.email, &auth.email, exclude)
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    state
        .reg_challenges
        .insert(auth.user_id.to_string(), reg_state);

    Ok(Json(ccr))
}

#[derive(Debug, Deserialize)]
pub struct RegisterFinishPayload {
    pub credential: RegisterPublicKeyCredential,
    pub name: Option<String>,
}

pub async fn register_finish(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<RegisterFinishPayload>,
) -> Result<impl IntoResponse, AppError> {
    let reg_state = state
        .reg_challenges
        .remove(&auth.user_id.to_string())
        .map(|(_, v)| v)
        .ok_or_else(|| {
            AppError::InvalidInput("no pending registration — call /start first".into())
        })?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&payload.credential, &reg_state)
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let name = payload.name.as_deref().unwrap_or("Passkey");

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;
    webauthn_service::save_credential(&txn, ctx.tenant_id, auth.user_id, &passkey, name).await?;
    txn.commit().await?;

    Ok(Json(json!({ "registered": true })))
}

// ── Authentication ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AuthStartPayload {
    pub email: String,
}

pub async fn authenticate_start(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<AuthStartPayload>,
) -> Result<impl IntoResponse, AppError> {
    let email_normalized = payload.email.trim().to_lowercase();
    let email_lookup = hefesto::hash_for_lookup(&email_normalized, &ctx.tenant_key)
        .map_err(AppError::CryptoError)?;

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;
    let user = user_service::find_by_email_lookup(&txn, &email_lookup)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let passkeys = webauthn_service::list_passkeys_for_user(&txn, ctx.tenant_id, user.id).await?;
    txn.commit().await?;

    if passkeys.is_empty() {
        return Err(AppError::InvalidInput(
            "no passkeys registered for this user".into(),
        ));
    }

    let (rcr, auth_state) = state
        .webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let token = Uuid::new_v4().to_string();
    state
        .auth_challenges
        .insert(token.clone(), (auth_state, user.id));

    Ok(Json(json!({
        "challenge": rcr,
        "token": token,
    })))
}

#[derive(Debug, Deserialize)]
pub struct AuthFinishPayload {
    pub token: String,
    pub credential: PublicKeyCredential,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

pub async fn authenticate_finish(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<AuthFinishPayload>,
) -> Result<impl IntoResponse, AppError> {
    let (auth_state, user_id) = state
        .auth_challenges
        .remove(&payload.token)
        .map(|(_, v)| v)
        .ok_or_else(|| AppError::InvalidInput("invalid or expired challenge token".into()))?;

    let auth_result = state
        .webauthn
        .finish_passkey_authentication(&payload.credential, &auth_state)
        .map_err(|_| AppError::Unauthorized)?;

    let settings = tenant_settings_service::get(&state.db, ctx.tenant_id).await?;

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let user = user_service::find_by_id(&txn, user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !user.is_active {
        return Err(AppError::Unauthorized);
    }
    if settings.require_email_verified && !user.email_verified {
        return Err(AppError::InvalidInput("email not verified".into()));
    }

    // Update passkey counter if needed
    if auth_result.needs_update() {
        let mut passkeys =
            webauthn_service::list_passkeys_for_user(&txn, ctx.tenant_id, user_id).await?;
        for pk in passkeys.iter_mut() {
            if pk.cred_id() == auth_result.cred_id() {
                pk.update_credential(&auth_result);
                webauthn_service::update_after_auth(&txn, ctx.tenant_id, pk).await?;
                break;
            }
        }
    }

    let email_plain = hefesto::decrypt(
        &user.email,
        &ctx.tenant_key,
        &state.config.master_encryption_key,
    )?;

    let roles = role_service::list_names_for_user(&txn, user.id, ctx.tenant_id)
        .await
        .unwrap_or_default();
    let permissions = permission_service::list_names_for_user(&txn, user.id, ctx.tenant_id)
        .await
        .unwrap_or_default();

    let access_token = token_service::generate_access_token(
        user.id,
        ctx.tenant_id,
        &email_plain,
        roles,
        permissions,
        std::collections::HashMap::new(),
        &state.config.jwt_secret,
        settings.access_token_ttl_minutes,
    )?;

    let refresh_token = token_service::generate_refresh_token();
    let token_hash = token_service::hash_refresh_token(&refresh_token);
    token_service::store_refresh_token(
        &txn,
        ctx.tenant_id,
        user.id,
        token_hash,
        settings.refresh_token_ttl_days,
    )
    .await?;

    txn.commit().await?;

    audit_service::record(
        state.db.clone(),
        ctx.tenant_id,
        Some(user.id),
        "login.webauthn.success",
        None,
        None,
    );

    session_service::create(
        &state.db,
        ctx.tenant_id,
        user.id,
        session_service::SessionData {
            email: email_plain,
            ip: None,
        },
        settings.refresh_token_ttl_days,
    )
    .await
    .ok();

    Ok(Json(TokenResponse {
        access_token,
        refresh_token,
        expires_in: settings.access_token_ttl_minutes * 60,
    }))
}
