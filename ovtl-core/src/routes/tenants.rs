use axum::{routing::post, Router};

use crate::{handlers::tenants::{create_tenant, list_tenants}, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tenants", post(create_tenant).get(list_tenants))
}
