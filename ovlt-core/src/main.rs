use migration::{Migrator, MigratorTrait};
use ovlt_core::{
    config::{self, Environment},
    db,
    entity::tenants,
    router::build_router,
    services::{
        bootstrap_service, jwk_service::JwkService, lockout_service, rate_limit_service,
        session_service, token_service,
    },
    state::AppState,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::net::SocketAddr;
use std::sync::Arc;
use sysinfo::{Pid, System};
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

    let db = db::connect(
        &config.database_url,
        config.db_max_connections,
        config.db_min_connections,
    )
    .await
    .unwrap_or_else(|e| {
        eprintln!("DB connection failed: {e}");
        std::process::exit(1);
    });

    Migrator::up(&db, None).await.unwrap_or_else(|e| {
        eprintln!("Migration failed: {e}");
        std::process::exit(1);
    });
    tracing::info!("migrations applied");

    bootstrap_service::run(&db, &config)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Bootstrap failed: {e}");
            std::process::exit(1);
        });

    let jwk = match (&config.rsa_private_key, &config.rsa_private_key_previous) {
        (Some(cur), Some(prev)) => JwkService::from_pem_b64_with_previous(cur, prev)
            .unwrap_or_else(|e| {
                eprintln!("RSA key error: {e}");
                std::process::exit(1);
            }),
        (Some(cur), None) => JwkService::from_pem_b64(cur).unwrap_or_else(|e| {
            eprintln!("RSA key error: {e}");
            std::process::exit(1);
        }),
        (None, _) => JwkService::generate(),
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
            match rate_limit_service::cleanup_expired(&db).await {
                Ok(n) => tracing::info!("cleanup: removed {n} expired rate limit buckets"),
                Err(e) => tracing::error!("rate limit cleanup error: {e}"),
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
