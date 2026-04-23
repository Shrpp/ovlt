use axum::{routing::{delete, post}, Router};

use crate::{
    handlers::clients::{create_client, delete_client, list_clients},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/clients", post(create_client).get(list_clients))
        .route("/clients/:id", delete(delete_client))
}
