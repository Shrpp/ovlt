use axum::{routing::{get, post}, Router};

use crate::{
    handlers::{
        oauth_as::{authorize, introspect, token},
        well_known::{discovery, jwks},
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/oauth/authorize", get(authorize))
        .route("/oauth/token", post(token))
        .route("/oauth/introspect", post(introspect))
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/.well-known/jwks.json", get(jwks))
}
