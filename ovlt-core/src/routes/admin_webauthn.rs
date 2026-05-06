use axum::{
    routing::{delete, get},
    Router,
};

use crate::{handlers::admin_webauthn, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/admin/users/:id/passkeys",
            get(admin_webauthn::list_passkeys),
        )
        .route(
            "/admin/users/:id/passkeys/:cred_id",
            delete(admin_webauthn::delete_passkey),
        )
}
