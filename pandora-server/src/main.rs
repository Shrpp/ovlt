use axum::{routing::get, Json, Router};
use pandora_server::{
    config::{self, Environment},
    db,
    middleware::{
        auth::auth_middleware,
        security::{rate_limit_middleware, security_headers_middleware},
        tenant::tenant_middleware,
    },
    routes,
    services::token_service,
    state::AppState,
};
use serde_json::json;
use std::net::SocketAddr;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let config = config::Config::from_env().unwrap_or_else(|e| {
        eprintln!("Config error: {e}");
        std::process::exit(1);
    });

    init_tracing(config.environment == Environment::Production);

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
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

fn build_router(state: AppState) -> Router {
    let cors = build_cors(&state.config.cors_allowed_origins);

    let public = Router::new().route("/health", get(health));

    // /auth/register, /auth/login, /auth/refresh — tenant + rate limit
    let auth_public = routes::auth::public_router()
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            tenant_middleware,
        ));

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
        .layer(axum::middleware::from_fn(security_headers_middleware))
        .layer(cors)
        .with_state(state)
}

fn build_cors(origins: &[String]) -> CorsLayer {
    if origins == ["*"] {
        CorsLayer::permissive()
    } else {
        let parsed: Vec<axum::http::HeaderValue> = origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        CorsLayer::new().allow_origin(AllowOrigin::list(parsed))
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

fn init_tracing(production: bool) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| "pandora_server=info".into());
    let registry = tracing_subscriber::registry().with(filter);
    if production {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }
}
