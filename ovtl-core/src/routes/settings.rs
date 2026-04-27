use axum::{routing::get, Router};

use crate::{
    handlers::admin_tenant_settings::{
        get_lockout, get_policy, get_registration, get_token_ttl,
        put_lockout, put_policy, put_registration, put_token_ttl,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings/password-policy", get(get_policy).put(put_policy))
        .route("/settings/lockout", get(get_lockout).put(put_lockout))
        .route("/settings/tokens", get(get_token_ttl).put(put_token_ttl))
        .route("/settings/registration", get(get_registration).put(put_registration))
}
