use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;
use webauthn_rs::prelude::{PasskeyAuthentication, PasskeyRegistration, Webauthn};

use crate::{config::Config, services::jwk_service::JwkService};

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
        }
    }
}
