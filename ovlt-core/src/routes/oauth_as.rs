use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    handlers::{oauth_as, oauth_revoke},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/oauth/authorize", get(oauth_as::authorize))
        .route("/oauth/token", post(oauth_as::token))
        .route("/oauth/introspect", post(oauth_as::introspect))
        .route("/oauth/revoke", post(oauth_revoke::revoke))
}
