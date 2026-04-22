use axum::{routing::get, Json, Router};
use pandora_server::{
    config, db,
    middleware::{auth::auth_middleware, tenant::tenant_middleware},
    routes,
    services::token_service,
    state::AppState,
};
use serde_json::json;
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

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

    let state = AppState::new(db.clone(), config.clone());

    // Background task: purge expired refresh tokens every 6 hours
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
            match token_service::cleanup_expired_tokens(&db).await {
                Ok(n) => tracing::info!("cleanup: removed {n} expired refresh tokens"),
                Err(e) => tracing::error!("cleanup error: {e}"),
            }
        }
    });

    let app = build_router(state);

    let addr: SocketAddr = format!("{}:{}", config.server_host, config.server_port)
        .parse()
        .expect("invalid server address");

    info!("Pandora running on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn build_router(state: AppState) -> Router {
    let public = Router::new().route("/health", get(health));

    // /auth/register, /auth/login, /auth/refresh — tenant only
    let auth_public = routes::auth::public_router().layer(
        axum::middleware::from_fn_with_state(state.clone(), tenant_middleware),
    );

    // /auth/logout, /auth/revoke + /users/me — tenant + JWT
    let auth_protected = routes::auth::protected_router()
        .merge(routes::user::router())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            tenant_middleware,
        ));

    // OAuth callbacks — no tenant middleware (tenant_id comes from state param)
    let oauth_callbacks = routes::auth::callback_router();

    Router::new()
        .merge(public)
        .merge(auth_public)
        .merge(auth_protected)
        .merge(oauth_callbacks)
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
