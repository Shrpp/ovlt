use axum::{
    extract::{Form, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response, Redirect},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    db,
    entity::authorization_codes,
    error::AppError,
    handlers::logout::get_session_cookie,
    services::{client_service, permission_service, role_service, session_service, tenant_service, token_service, user_service},
    state::AppState,
};

// ── /oauth/authorize ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AuthorizeParams {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub scope: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub state: Option<String>,
    pub nonce: Option<String>,
}

pub async fn authorize(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<AuthorizeParams>,
) -> Result<impl IntoResponse, AppError> {
    // Accept Bearer token (API-first) OR session cookie (browser SSO).
    let (tenant_id, user_id) = if let Some(bearer) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        let claims = token_service::validate_access_token(bearer, &state.config.jwt_secret)?;
        if token_service::is_jti_revoked(&state.db, &claims.jti).await? {
            return Err(AppError::Unauthorized);
        }
        let tid = Uuid::parse_str(&claims.tid)
            .map_err(|_| AppError::TokenError("invalid tid".into()))?;
        let uid = Uuid::parse_str(&claims.sub)
            .map_err(|_| AppError::TokenError("invalid sub".into()))?;
        (tid, uid)
    } else if let Some(session_id) = get_session_cookie(&headers) {
        let session = session_service::find_valid(&state.db, &session_id)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let _ = session_service::touch(&state.db, &session_id).await;
        (session.tenant_id, session.user_id)
    } else {
        return Err(AppError::Unauthorized);
    };

    if params.response_type != "code" {
        return Err(AppError::InvalidInput("response_type must be code".into()));
    }
    if params.code_challenge_method != "S256" {
        return Err(AppError::InvalidInput(
            "code_challenge_method must be S256".into(),
        ));
    }
    if params.code_challenge.is_empty() {
        return Err(AppError::InvalidInput("code_challenge required".into()));
    }

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;

    let client = client_service::find_by_client_id(&txn, &params.client_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if !client.is_active {
        txn.commit().await?;
        return Err(AppError::Unauthorized);
    }

    if !client_service::redirect_uri_allowed(&client, &params.redirect_uri) {
        txn.commit().await?;
        return Err(AppError::InvalidInput("redirect_uri not allowed".into()));
    }

    let code = hex::encode(Sha256::digest(Uuid::new_v4().to_string().as_bytes()))
        + &hex::encode(Sha256::digest(Uuid::new_v4().to_string().as_bytes()));

    let scopes = params
        .scope
        .as_deref()
        .unwrap_or("openid")
        .split_whitespace()
        .map(|s| s.to_owned())
        .collect::<Vec<_>>();

    let expires_at = (Utc::now() + chrono::Duration::minutes(10)).fixed_offset();

    authorization_codes::ActiveModel {
        code: Set(code.clone()),
        tenant_id: Set(tenant_id),
        client_id: Set(params.client_id),
        user_id: Set(user_id),
        redirect_uri: Set(params.redirect_uri.clone()),
        scopes: Set(serde_json::json!(scopes)),
        code_challenge: Set(params.code_challenge),
        nonce: Set(params.nonce),
        expires_at: Set(expires_at),
        used_at: Set(None),
    }
    .insert(&txn)
    .await?;

    txn.commit().await?;

    let mut redirect_url = format!("{}?code={}", params.redirect_uri, code);
    if let Some(s) = &params.state {
        redirect_url.push_str(&format!("&state={s}"));
    }

    Ok(Redirect::to(&redirect_url))
}

// ── /oauth/token ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub code_verifier: Option<String>,
    pub scope: Option<String>,      // for client_credentials
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub scope: String,
}

#[derive(Serialize)]
struct IdTokenClaims {
    iss: String,
    sub: String,
    aud: String,
    iat: i64,
    exp: i64,
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<String>,
}

pub async fn token(
    State(state): State<AppState>,
    Form(req): Form<TokenRequest>,
) -> Result<Response, AppError> {
    match req.grant_type.as_str() {
        "authorization_code" => token_authorization_code(state, req).await.map(IntoResponse::into_response),
        "client_credentials" => token_client_credentials(state, req).await.map(IntoResponse::into_response),
        _ => Err(AppError::InvalidInput("unsupported grant_type".into())),
    }
}

async fn token_authorization_code(state: AppState, req: TokenRequest) -> Result<impl IntoResponse, AppError> {
    let code = req.code.as_deref().ok_or_else(|| AppError::InvalidInput("code required".into()))?;
    let redirect_uri = req.redirect_uri.as_deref().ok_or_else(|| AppError::InvalidInput("redirect_uri required".into()))?;
    let code_verifier = req.code_verifier.as_deref().ok_or_else(|| AppError::InvalidInput("code_verifier required".into()))?;

    // 1. Fetch the authorization code (no RLS — codes table has no policy).
    let auth_code = authorization_codes::Entity::find_by_id(code)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::InvalidInput("invalid code".into()))?;

    let now = Utc::now().fixed_offset();
    if auth_code.expires_at < now {
        return Err(AppError::InvalidInput("code expired".into()));
    }
    if auth_code.used_at.is_some() {
        return Err(AppError::InvalidInput("code already used".into()));
    }
    if auth_code.redirect_uri != redirect_uri {
        return Err(AppError::InvalidInput("redirect_uri mismatch".into()));
    }

    // 2. Verify PKCE S256: BASE64URL(SHA256(code_verifier)) == stored code_challenge.
    let computed = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
    if computed != auth_code.code_challenge {
        return Err(AppError::InvalidInput("invalid code_verifier".into()));
    }

    let tenant_id = auth_code.tenant_id;
    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;

    // 3. Validate client (RLS scopes this to the right tenant).
    let client = client_service::find_by_client_id(&txn, &req.client_id)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !client.is_active {
        txn.commit().await?;
        return Err(AppError::Unauthorized);
    }
    if client.client_id != auth_code.client_id {
        txn.commit().await?;
        return Err(AppError::Unauthorized);
    }

    if client.is_confidential {
        let secret = req
            .client_secret
            .as_deref()
            .ok_or(AppError::Unauthorized)?;
        if !client_service::verify_secret(secret, &client.client_secret) {
            txn.commit().await?;
            return Err(AppError::Unauthorized);
        }
    }

    // 4. Mark code used.
    let mut active: authorization_codes::ActiveModel = auth_code.clone().into();
    active.used_at = Set(Some(now));
    active.update(&txn).await?;

    // 5. Get tenant key to decrypt user email for id_token.
    let tenant_record = tenant_service::find_active(&state.db, tenant_id).await?;
    let tenant_key = hefesto::decrypt(
        &tenant_record.encryption_key_encrypted,
        &state.config.tenant_wrap_key,
        &state.config.master_encryption_key,
    )?;

    let user = user_service::find_by_id(&txn, auth_code.user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let email_plain = hefesto::decrypt(
        &user.email,
        &tenant_key,
        &state.config.master_encryption_key,
    )?;

    // 6. Issue access + refresh tokens.
    let roles = role_service::list_names_for_user(&txn, user.id, tenant_id)
        .await
        .unwrap_or_default();
    let permissions = permission_service::list_names_for_user(&txn, user.id, tenant_id)
        .await
        .unwrap_or_default();

    let access_token = token_service::generate_access_token(
        user.id,
        tenant_id,
        &email_plain,
        roles,
        permissions,
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

    // 7. Sign id_token (RS256).
    let scope_str = client_service::scopes_to_vec(&auth_code.scopes).join(" ");
    let id_claims = IdTokenClaims {
        iss: state.config.ovtl_issuer.clone(),
        sub: user.id.to_string(),
        aud: req.client_id.clone(),
        iat: Utc::now().timestamp(),
        exp: Utc::now().timestamp() + 3600,
        email: email_plain,
        nonce: auth_code.nonce,
    };

    let id_token = state
        .jwk
        .sign_id_token(&id_claims)
        .map_err(|e| AppError::TokenError(e.to_string()))?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token: Some(refresh_token),
        id_token: Some(id_token),
        token_type: "Bearer",
        expires_in: state.config.jwt_expiration_minutes * 60,
        scope: scope_str,
    }))
}

async fn token_client_credentials(state: AppState, req: TokenRequest) -> Result<impl IntoResponse, AppError> {
    // 1. Find client globally (cross-tenant, bypasses RLS via superuser connection).
    let client = client_service::find_by_client_id_global(&state.db, &req.client_id)
        .await?
        .ok_or(AppError::Unauthorized)?;

    // 2. Must be confidential and support client_credentials grant.
    if !client.is_confidential {
        return Err(AppError::Unauthorized);
    }
    let grant_types = client_service::scopes_to_vec(&client.grant_types);
    if !grant_types.iter().any(|g| g == "client_credentials") {
        return Err(AppError::InvalidInput("client does not support client_credentials".into()));
    }

    // 3. Verify secret.
    let secret = req.client_secret.as_deref().ok_or(AppError::Unauthorized)?;
    if !client_service::verify_secret(secret, &client.client_secret) {
        return Err(AppError::Unauthorized);
    }

    // 4. Resolve scopes (intersection of requested + registered).
    let registered_scopes = client_service::scopes_to_vec(&client.scopes);
    let requested_scopes: Vec<String> = req.scope
        .as_deref()
        .map(|s| s.split_whitespace().map(|s| s.to_owned()).collect())
        .unwrap_or_else(|| registered_scopes.clone());
    let final_scopes: Vec<String> = requested_scopes
        .into_iter()
        .filter(|s| registered_scopes.contains(s))
        .collect();

    // 5. Issue access token (no user context, sub = client_id).
    let access_token = token_service::generate_client_access_token(
        &client.client_id,
        client.tenant_id,
        &final_scopes,
        &state.config.jwt_secret,
        state.config.jwt_expiration_minutes,
    )?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token: None,
        id_token: None,
        token_type: "Bearer",
        expires_in: state.config.jwt_expiration_minutes * 60,
        scope: final_scopes.join(" "),
    }))
}

// ── /oauth/introspect ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct IntrospectRequest {
    pub token: String,
}

pub async fn introspect(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(req): Form<IntrospectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let Some(expected_key) = &state.config.admin_key else {
        return Err(AppError::NotFound);
    };
    let provided = headers
        .get("x-ovtl-admin-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if provided != expected_key {
        return Err(AppError::Unauthorized);
    }

    match token_service::validate_access_token(&req.token, &state.config.jwt_secret) {
        Ok(claims) => Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "active": true,
                "sub": claims.sub,
                "tid": claims.tid,
                "email": claims.email,
                "jti": claims.jti,
                "exp": claims.exp,
                "iat": claims.iat,
            })),
        )),
        Err(_) => Ok((StatusCode::OK, Json(serde_json::json!({ "active": false })))),
    }
}
