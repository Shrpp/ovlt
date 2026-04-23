pub use sea_orm_migration::prelude::*;

mod m20240101_000001_create_tenants;
mod m20240101_000002_create_users;
mod m20240101_000003_create_refresh_tokens;
mod m20240101_000004_create_oauth_accounts;
mod m20240101_000005_security_hardening;
mod m20240101_000006_oauth_clients;
mod m20240101_000007_authorization_codes;
mod m20240101_000008_sessions;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_create_tenants::Migration),
            Box::new(m20240101_000002_create_users::Migration),
            Box::new(m20240101_000003_create_refresh_tokens::Migration),
            Box::new(m20240101_000004_create_oauth_accounts::Migration),
            Box::new(m20240101_000005_security_hardening::Migration),
            Box::new(m20240101_000006_oauth_clients::Migration),
            Box::new(m20240101_000007_authorization_codes::Migration),
            Box::new(m20240101_000008_sessions::Migration),
        ]
    }
}
