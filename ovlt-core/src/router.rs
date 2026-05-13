use axum::{routing::get, Router};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::{
    handlers::well_known,
    middleware::{
        auth::auth_middleware,
        security::{rate_limit_middleware, security_headers_middleware},
        tenant::tenant_middleware,
    },
    openapi, routes,
    state::AppState,
};

pub fn build_router(state: AppState) -> Router {
    let cors = build_cors(&state.config.cors_allowed_origins);

    let public = Router::new().route("/health", get(health));

    let auth_public = routes::auth::public_router()
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            tenant_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
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
        .merge(openapi::swagger_router())
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

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}
