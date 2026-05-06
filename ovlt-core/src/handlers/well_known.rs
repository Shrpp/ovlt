use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/.well-known/openid-configuration",
    tag = "oauth",
    responses(
        (status = 200, description = "OpenID Connect discovery document"),
    )
)]
pub async fn discovery(State(state): State<AppState>) -> impl IntoResponse {
    let base = &state.config.ovlt_issuer;
    Json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth/authorize"),
        "token_endpoint": format!("{base}/oauth/token"),
        "jwks_uri": format!("{base}/.well-known/jwks.json"),
        "introspection_endpoint": format!("{base}/oauth/introspect"),
        "revocation_endpoint": format!("{base}/oauth/revoke"),
        "end_session_endpoint": format!("{base}/auth/logout"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token", "client_credentials"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "scopes_supported": ["openid", "email", "profile"],
        "token_endpoint_auth_methods_supported": ["client_secret_post", "none"],
        "code_challenge_methods_supported": ["S256"],
        "claims_supported": ["sub", "iss", "aud", "iat", "exp", "email", "nonce"]
    }))
}

#[utoipa::path(
    get,
    path = "/.well-known/jwks.json",
    tag = "oauth",
    responses(
        (status = 200, description = "JSON Web Key Set"),
    )
)]
pub async fn jwks(State(state): State<AppState>) -> impl IntoResponse {
    axum::response::Response::builder()
        .header("Content-Type", "application/json")
        .body(state.jwk.jwks_json.clone())
        .unwrap()
}
