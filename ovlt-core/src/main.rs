use axum::{routing::get, Json, Router};
use migration::{Migrator, MigratorTrait};
use ovlt_core::{
    config::{self, Environment},
    db,
    entity::tenants,
    handlers::well_known,
    middleware::{
        auth::auth_middleware,
        security::{rate_limit_middleware, security_headers_middleware},
        tenant::tenant_middleware,
    },
    routes,
    services::{
        bootstrap_service, jwk_service::JwkService, lockout_service, session_service, token_service,
    },
    state::AppState,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use sysinfo::{Pid, System};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use webauthn_rs::prelude::{Url, WebauthnBuilder};

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

    if std::env::args().any(|a| a == "--migrate") {
        Migrator::up(&db, None).await.unwrap_or_else(|e| {
            eprintln!("Migration failed: {e}");
            std::process::exit(1);
        });
        tracing::info!("migrations applied");
    }

    bootstrap_service::run(&db, &config)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Bootstrap failed: {e}");
            std::process::exit(1);
        });

    let jwk = match &config.rsa_private_key {
        Some(b64) => JwkService::from_pem_b64(b64).unwrap_or_else(|e| {
            eprintln!("RSA key error: {e}");
            std::process::exit(1);
        }),
        None => JwkService::generate(),
    };

    let master_slug = config.bootstrap_tenant_slug.as_deref().unwrap_or("master");
    let master_tenant_id = tenants::Entity::find()
        .filter(tenants::Column::Slug.eq(master_slug))
        .one(&db)
        .await
        .ok()
        .flatten()
        .map(|t| t.id);

    let rp_origin = Url::parse(&config.ovlt_issuer).unwrap_or_else(|_| {
        eprintln!("OVLT_ISSUER is not a valid URL");
        std::process::exit(1);
    });
    let rp_id = rp_origin.host_str().unwrap_or("localhost").to_string();
    let webauthn = Arc::new(
        WebauthnBuilder::new(&rp_id, &rp_origin)
            .unwrap_or_else(|e| {
                eprintln!("WebAuthn init error: {e}");
                std::process::exit(1);
            })
            .rp_name("OVLT")
            .build()
            .unwrap_or_else(|e| {
                eprintln!("WebAuthn build error: {e}");
                std::process::exit(1);
            }),
    );

    let state = AppState::new(db.clone(), config.clone(), jwk, master_tenant_id, webauthn);

    // Background cleanup every 6 hours
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
            match token_service::cleanup_expired_tokens(&db).await {
                Ok(n) => tracing::info!("cleanup: removed {n} expired refresh tokens"),
                Err(e) => tracing::error!("cleanup error: {e}"),
            }
            match lockout_service::cleanup_old_attempts(&db).await {
                Ok(n) => tracing::info!("cleanup: removed {n} stale login attempts"),
                Err(e) => tracing::error!("lockout cleanup error: {e}"),
            }
            match token_service::cleanup_expired_jtis(&db).await {
                Ok(n) => tracing::info!("cleanup: removed {n} expired JTIs"),
                Err(e) => tracing::error!("JTI cleanup error: {e}"),
            }
            match session_service::cleanup_expired(&db).await {
                Ok(n) => tracing::info!("cleanup: removed {n} expired sessions"),
                Err(e) => tracing::error!("session cleanup error: {e}"),
            }
        }
    });

    let app = build_router(state);

    let addr: SocketAddr = format!("{}:{}", config.server_host, config.server_port)
        .parse()
        .expect("invalid server address");

    info!("OVLT running on {addr}");
    log_startup_stats();

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

    let auth_public = routes::auth::public_router()
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            tenant_middleware,
        ));

    let auth_universal = routes::auth::universal_router().layer(
        axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware),
    );

    let auth_protected = routes::auth::protected_router()
        .merge(routes::user::router())
        .merge(routes::settings::router())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            tenant_middleware,
        ));

    let oauth_callbacks = routes::auth::callback_router();

    let admin = routes::tenants::router()
        .merge(routes::clients::router())
        .merge(routes::admin_users::router())
        .merge(routes::admin_sessions::router())
        .merge(routes::admin_roles::router())
        .merge(routes::admin_permissions::router())
        .merge(routes::admin_identity_providers::router())
        .merge(routes::admin_smtp::router())
        .merge(routes::admin_webauthn::router())
        .merge(routes::audit_log::router());

    let well_known_router = Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(well_known::discovery),
        )
        .route("/.well-known/jwks.json", get(well_known::jwks));

    let oauth_as = routes::oauth_as::router();

    Router::new()
        .merge(public)
        .merge(auth_universal)
        .merge(auth_public)
        .merge(auth_protected)
        .merge(oauth_callbacks)
        .merge(admin)
        .merge(well_known_router)
        .merge(oauth_as)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            security_headers_middleware,
        ))
        .layer(cors)
        .with_state(state)
}

fn build_cors(origins: &[String]) -> CorsLayer {
    if origins == ["*"] {
        CorsLayer::permissive()
    } else {
        let parsed: Vec<axum::http::HeaderValue> =
            origins.iter().filter_map(|o| o.parse().ok()).collect();
        CorsLayer::new().allow_origin(AllowOrigin::list(parsed))
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

fn log_startup_stats() {
    let pid = std::process::id();
    let mut sys = System::new();
    sys.refresh_processes();

    let (rss_mb, threads) = sys
        .process(Pid::from(pid as usize))
        .map(|p| (p.memory() / 1024 / 1024, p.tasks().map_or(0, |t| t.len())))
        .unwrap_or((0, 0));

    let cpus = sys.cpus().len();

    info!(
        rss_mb,
        threads,
        cpus,
        version = env!("CARGO_PKG_VERSION"),
        "startup stats"
    );
}

fn init_tracing(production: bool) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "ovlt_core=info".into());
    let registry = tracing_subscriber::registry().with(filter);
    if production {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }
}
