use sea_orm::DatabaseConnection;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use crate::{config::Config, services::jwk_service::JwkService};

pub type RateLimiterStore = Arc<Mutex<HashMap<String, Vec<Instant>>>>;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub config: Config,
    pub rate_limiter: RateLimiterStore,
    pub jwk: Arc<JwkService>,
}

impl AppState {
    pub fn new(db: DatabaseConnection, config: Config, jwk: JwkService) -> Self {
        Self {
            db,
            config,
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
            jwk: Arc::new(jwk),
        }
    }
}
