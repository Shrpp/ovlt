use axum::{routing::get, Router};

use crate::{handlers::admin_smtp, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/admin/smtp",
        get(admin_smtp::get_smtp).put(admin_smtp::put_smtp),
    )
}
