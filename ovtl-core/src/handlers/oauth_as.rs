use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect},
    Form, Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use jsonwebtoken::{Algorithm, Header};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    db,
    entity::authorization_codes,
    error::AppError,
    services::{client_service, token_service, user_service},
    state::AppState,
};

// ─── Authorize ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub scope: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub state: Option<String>,
    pub nonce: Option<String>,
}

pub async fn authorize(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<AuthorizeQuery>,
) -> Result<impl IntoResponse, AppError> {
    // Validate response_type and PKCE method upfront.
    if params.response_type != "code" {
        return Err(AppError::InvalidInput(
            "response_type must be 'code'".into(),
        ));
    }
    if params.code_challenge_method != "S256" {
        return Err(AppError::InvalidInput(
            "code_challenge_method must be 'S256'".into(),
        ));
    }

    // Extract user from Bearer token.
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    let claims = token_service::validate_access_token(bearer, &state.config.jwt_secret)?;
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let tenant_id: Uuid = claims.tid.parse().map_err(|_| AppError::Unauthorized)?;

    // Validate client within tenant context.
    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let client = client_service::find_by_client_id(&txn, &params.client_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if !client_service::redirect_uri_allowed(&client, &params.redirect_uri) {
        return Err(AppError::InvalidInput("redirect_uri not allowed".into()));
    }

    let requested_scopes: Vec<&str> = params.scope.split_whitespace().collect();
    if !client_service::scopes_allowed(&client, &requested_scopes) {
        return Err(AppError::InvalidInput("scope not allowed for this client".into()));
    }

    // Generate authorization code.
    let code = hex::encode(Uuid::new_v4().as_bytes()) + &hex::encode(Uuid::new_v4().as_bytes());
    let expires_at = (Utc::now() + chrono::Duration::minutes(5)).fixed_offset();

    authorization_codes::ActiveModel {
        code: Set(code.clone()),
        tenant_id: Set(tenant_id),
        client_id: Set(params.client_id),
        user_id: Set(user_id),
        redirect_uri: Set(params.redirect_uri.clone()),
        scopes: Set(params.scope),
        code_challenge: Set(params.code_challenge),
        nonce: Set(params.nonce),
        expires_at: Set(expires_at),
        used_at: Set(None),
    }
    .insert(&txn)
    .await?;

    txn.commit().await?;

    let mut location = format!("{}?code={}", params.redirect_uri, code);
    if let Some(s) = &params.state {
        location.push_str(&format!("&state={s}"));
    }

    Ok(Redirect::temporary(&location))
}

// ─── Token ───────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TokenForm {
    pub grant_type: String,
    pub code: String,
    pub redirect_uri: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub code_verifier: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub scope: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct IdClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub iat: i64,
    pub exp: i64,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

fn verify_pkce(code_verifier: &str, code_challenge: &str) -> bool {
    let hash = Sha256::digest(code_verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash) == code_challenge
}

pub async fn token(
    State(state): State<AppState>,
    Form(form): Form<TokenForm>,
) -> Result<impl IntoResponse, AppError> {
    if form.grant_type != "authorization_code" {
        return Err(AppError::InvalidInput(
            "grant_type must be 'authorization_code'".into(),
        ));
    }

    let now = Utc::now();

    // Fetch and validate the authorization code (no tenant context — cross-tenant lookup).
    let auth_code = authorization_codes::Entity::find()
        .filter(authorization_codes::Column::Code.eq(&form.code))
        .filter(authorization_codes::Column::ClientId.eq(&form.client_id))
        .filter(authorization_codes::Column::UsedAt.is_null())
        .filter(authorization_codes::Column::ExpiresAt.gt(now.fixed_offset()))
        .one(&state.db)
        .await?
        .ok_or(AppError::InvalidInput("invalid or expired code".into()))?;

    if auth_code.redirect_uri != form.redirect_uri {
        return Err(AppError::InvalidInput("redirect_uri mismatch".into()));
    }
    if !verify_pkce(&form.code_verifier, &auth_code.code_challenge) {
        return Err(AppError::InvalidInput("code_verifier invalid".into()));
    }

    let tenant_id = auth_code.tenant_id;

    // Validate client.
    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let client = client_service::find_by_client_id(&txn, &form.client_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if client.is_confidential {
        let secret = form
            .client_secret
            .as_deref()
            .ok_or(AppError::Unauthorized)?;
        if !client_service::verify_secret(&client, secret) {
            return Err(AppError::Unauthorized);
        }
    }

    // Mark code as used (single-use enforcement).
    let mut active: authorization_codes::ActiveModel = auth_code.clone().into();
    active.used_at = Set(Some(now.fixed_offset()));
    active.update(&txn).await?;

    // Load user to get email for id_token.
    let user = user_service::find_by_id(&txn, auth_code.user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Generate standard tokens.
    let access_token = token_service::generate_access_token(
        auth_code.user_id,
        tenant_id,
        &user.email,
        &state.config.jwt_secret,
        state.config.jwt_expiration_minutes,
    )?;

    let refresh_token = token_service::generate_refresh_token();
    let token_hash = token_service::hash_refresh_token(&refresh_token);
    token_service::store_refresh_token(
        &txn,
        tenant_id,
        auth_code.user_id,
        token_hash,
        state.config.refresh_token_expiration_days,
    )
    .await?;

    // Fetch the decrypted email properly via the tenant key.
    let tenant_rec = crate::services::tenant_service::find_active(&state.db, tenant_id).await?;
    let tenant_key = hefesto::decrypt(
        &tenant_rec.encryption_key_encrypted,
        &state.config.tenant_wrap_key,
        &state.config.master_encryption_key,
    )?;
    let email_decrypted = hefesto::decrypt(
        &user.email,
        &tenant_key,
        &state.config.master_encryption_key,
    )?;

    txn.commit().await?;

    // Build id_token (RS256).
    let id_claims = IdClaims {
        iss: state.config.ovtl_issuer.clone(),
        sub: auth_code.user_id.to_string(),
        aud: form.client_id.clone(),
        iat: now.timestamp(),
        exp: now.timestamp() + state.config.jwt_expiration_minutes * 60,
        email: email_decrypted,
        nonce: auth_code.nonce,
    };

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(state.jwk.kid.clone());

    let id_token = jsonwebtoken::encode(&header, &id_claims, &state.jwk.encoding_key)
        .map_err(|e| AppError::TokenError(e.to_string()))?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token,
        id_token,
        token_type: "Bearer".into(),
        expires_in: state.config.jwt_expiration_minutes * 60,
        scope: auth_code.scopes,
    }))
}

// ─── Introspect ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct IntrospectForm {
    pub token: String,
}

pub async fn introspect(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<IntrospectForm>,
) -> impl IntoResponse {
    // Require admin key for introspection.
    let valid_admin = state
        .config
        .admin_key
        .as_deref()
        .map(|k| {
            headers
                .get("x-ovtl-admin-key")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                == k
        })
        .unwrap_or(false);

    if !valid_admin {
        return Json(serde_json::json!({ "active": false }));
    }

    match token_service::validate_access_token(&form.token, &state.config.jwt_secret) {
        Ok(claims) => Json(serde_json::json!({
            "active": true,
            "sub": claims.sub,
            "tid": claims.tid,
            "email": claims.email,
            "exp": claims.exp,
            "jti": claims.jti,
        })),
        Err(_) => Json(serde_json::json!({ "active": false })),
    }
}
