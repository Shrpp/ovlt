use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    handlers::{
        forgot_password::forgot_password,
        login::login,
        logout::logout,
        mfa::{mfa_challenge, mfa_confirm, mfa_disable, mfa_setup},
        oauth::{authorize, callback},
        refresh::refresh,
        register::register,
        reset_password::reset_password,
        revoke::revoke,
        verify_email::verify_email,
    },
    state::AppState,
};

/// Routes that need only tenant context (no JWT).
pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/auth/forgot-password", post(forgot_password))
        .route("/auth/reset-password", post(reset_password))
        .route("/auth/verify-otp", post(verify_email))
        .route("/auth/mfa/challenge", post(mfa_challenge))
        // OAuth authorize — tenant header required (client sets it)
        .route("/auth/:provider", get(authorize))
}

/// Routes that need tenant + JWT.
pub fn protected_router() -> Router<AppState> {
    Router::new()
        .route("/auth/logout", post(logout))
        .route("/auth/revoke", post(revoke))
        .route("/auth/mfa/setup", post(mfa_setup))
        .route("/auth/mfa/confirm", post(mfa_confirm))
        .route("/auth/mfa/disable", post(mfa_disable))
}

/// OAuth callbacks — no tenant header; tenant extracted from state param.
pub fn callback_router() -> Router<AppState> {
    Router::new().route("/auth/:provider/callback", get(callback))
}
