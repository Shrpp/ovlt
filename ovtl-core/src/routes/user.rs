use axum::{routing::get, Router};

use crate::{handlers::me::me, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/users/me", get(me))
}
