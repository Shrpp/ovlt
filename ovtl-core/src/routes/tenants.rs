use axum::{routing::{get, post}, Router};

use crate::{handlers::tenants::{create_tenant, list_tenant_slugs, list_tenants}, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tenants", post(create_tenant).get(list_tenants))
        .route("/tenants/slugs", get(list_tenant_slugs))
}
