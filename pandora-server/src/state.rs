use sea_orm::DatabaseConnection;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use crate::config::Config;

/// Sliding-window per-IP rate limit store.
pub type RateLimiterStore = Arc<Mutex<HashMap<String, Vec<Instant>>>>;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub config: Config,
    pub rate_limiter: RateLimiterStore,
}

impl AppState {
    pub fn new(db: DatabaseConnection, config: Config) -> Self {
        Self {
            db,
            config,
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
