use axum::{routing::get, Json, Router};
use serde_json::json;
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod db;
mod entity;
mod error;
mod middleware;
mod services;
mod state;

use crate::middleware::tenant::tenant_middleware;
use state::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    init_tracing();

    let config = config::Config::from_env().unwrap_or_else(|e| {
        eprintln!("Config error: {e}");
        std::process::exit(1);
    });

    let db = db::connect(&config.database_url).await.unwrap_or_else(|e| {
        eprintln!("DB connection failed: {e}");
        std::process::exit(1);
    });

    let state = AppState::new(db, config.clone());

    let app = build_router(state);

    let addr: SocketAddr = format!("{}:{}", config.server_host, config.server_port)
        .parse()
        .expect("invalid server address");

    info!("Pandora running on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn build_router(state: AppState) -> Router {
    // Public — no tenant required
    let public = Router::new().route("/health", get(health));

    // Protected — tenant middleware applied; auth routes added in Phase 5
    let protected = Router::new().layer(axum::middleware::from_fn_with_state(
        state.clone(),
        tenant_middleware,
    ));

    Router::new()
        .merge(public)
        .merge(protected)
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "pandora_server=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
}
