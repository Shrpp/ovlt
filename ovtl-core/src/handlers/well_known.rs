use axum::{extract::State, response::IntoResponse, Json};

use crate::state::AppState;

pub async fn discovery(State(state): State<AppState>) -> impl IntoResponse {
    let base = &state.config.ovtl_issuer;
    Json(serde_json::json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth/authorize"),
        "token_endpoint": format!("{base}/oauth/token"),
        "jwks_uri": format!("{base}/.well-known/jwks.json"),
        "introspection_endpoint": format!("{base}/oauth/introspect"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "scopes_supported": ["openid", "email", "profile"],
        "token_endpoint_auth_methods_supported": ["client_secret_post", "none"],
        "code_challenge_methods_supported": ["S256"],
        "claims_supported": ["sub", "iss", "aud", "iat", "exp", "email", "nonce"]
    }))
}

pub async fn jwks(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.jwk.jwks_json.clone())
}
