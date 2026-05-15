use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;
use webauthn_rs::prelude::{PasskeyAuthentication, PasskeyRegistration, Webauthn};
use zeroize::Zeroizing;

use crate::{config::Config, services::jwk_service::JwkService};

const TENANT_KEY_TTL: Duration = Duration::from_secs(300);

pub struct CachedTenantKey {
    key: Zeroizing<String>,
    expires_at: Instant,
}

impl CachedTenantKey {
    pub fn new(key: String) -> Self {
        Self {
            key: Zeroizing::new(key),
            expires_at: Instant::now() + TENANT_KEY_TTL,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.expires_at > Instant::now()
    }

    pub fn get(&self) -> &str {
        self.key.as_str()
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub config: Config,
    pub jwk: Arc<JwkService>,
    pub master_tenant_id: Option<Uuid>,
    pub webauthn: Arc<Webauthn>,
    /// Pending passkey registration challenges, keyed by user_id string.
    pub reg_challenges: Arc<DashMap<String, PasskeyRegistration>>,
    /// Pending passkey authentication challenges, keyed by random challenge token.
    /// Value: (auth_state, user_id)
    pub auth_challenges: Arc<DashMap<String, (PasskeyAuthentication, Uuid)>>,
    /// Decrypted tenant data keys, cached for up to 5 min. Zeroed on eviction.
    pub tenant_key_cache: Arc<DashMap<Uuid, CachedTenantKey>>,
}

impl AppState {
    pub fn new(
        db: DatabaseConnection,
        config: Config,
        jwk: JwkService,
        master_tenant_id: Option<Uuid>,
        webauthn: Arc<Webauthn>,
    ) -> Self {
        Self {
            db,
            config,
            jwk: Arc::new(jwk),
            master_tenant_id,
            webauthn,
            reg_challenges: Arc::new(DashMap::new()),
            auth_challenges: Arc::new(DashMap::new()),
            tenant_key_cache: Arc::new(DashMap::new()),
        }
    }
}
