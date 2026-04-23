use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderName, HeaderValue, Request},
    middleware::Next,
    response::IntoResponse,
};
use std::net::SocketAddr;
use std::time::Instant;

use crate::{error::AppError, state::AppState};

pub async fn security_headers_middleware(
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    );
    response
}

const RATE_LIMIT_MAX: usize = 20;
const RATE_LIMIT_WINDOW_SECS: u64 = 60;

pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, AppError> {
    let ip = addr.ip().to_string();
    let now = Instant::now();
    let window = std::time::Duration::from_secs(RATE_LIMIT_WINDOW_SECS);

    let allowed = {
        let mut store = state.rate_limiter.lock().unwrap();
        let timestamps = store.entry(ip).or_default();
        timestamps.retain(|t| now.duration_since(*t) < window);
        if timestamps.len() >= RATE_LIMIT_MAX {
            false
        } else {
            timestamps.push(now);
            true
        }
    };

    if !allowed {
        return Err(AppError::TooManyRequests);
    }

    Ok(next.run(request).await)
}
