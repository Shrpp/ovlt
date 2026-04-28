use axum::{
    routing::{delete, post},
    Router,
};

use crate::{handlers::clients, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/clients",
            post(clients::create_client).get(clients::list_clients),
        )
        .route(
            "/clients/:id",
            delete(clients::deactivate_client).put(clients::update_client),
        )
}
