use axum::{routing::{get, post}, Router};

use crate::{
    handlers::{
        login::login,
        logout::logout,
        oauth::{authorize, callback},
        refresh::refresh,
        register::register,
        revoke::revoke,
    },
    state::AppState,
};

/// Routes that need only tenant context (no JWT).
pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        // OAuth authorize — tenant header required (client sets it)
        .route("/auth/:provider", get(authorize))
}

/// Routes that need tenant + JWT.
pub fn protected_router() -> Router<AppState> {
    Router::new()
        .route("/auth/logout", post(logout))
        .route("/auth/revoke", post(revoke))
}

/// OAuth callbacks — no tenant header; tenant extracted from state param.
pub fn callback_router() -> Router<AppState> {
    Router::new().route("/auth/:provider/callback", get(callback))
}
