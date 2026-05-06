pub use sea_orm_migration::prelude::*;

mod m20240101_000001_create_tenants;
mod m20240101_000002_create_users;
mod m20240101_000003_create_refresh_tokens;
mod m20240101_000004_create_oauth_accounts;
mod m20240101_000005_security_hardening;
mod m20240101_000006_oauth_clients;
mod m20240101_000007_authorization_codes;
mod m20240101_000008_sessions;
mod m20240101_000009_fix_rls_force;
mod m20240101_000010_revoked_jtis;
mod m20240101_000011_sessions_last_seen;
mod m20240101_000012_roles;
mod m20240101_000013_permissions;
mod m20240101_000014_sessions_rls;
mod m20240101_000015_rls_app_role;
mod m20240101_000016_user_lifecycle;
mod m20240101_000017_tenant_settings;
mod m20240101_000018_client_roles;
mod m20240101_000019_client_token_ttl;
mod m20240101_000020_identity_providers;
mod m20240101_000021_totp_secrets;
mod m20240101_000022_rename_role;
mod m20240101_000023_tenant_smtp_config;
mod m20240101_000024_webauthn_credentials;

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
            Box::new(m20240101_000009_fix_rls_force::Migration),
            Box::new(m20240101_000010_revoked_jtis::Migration),
            Box::new(m20240101_000011_sessions_last_seen::Migration),
            Box::new(m20240101_000012_roles::Migration),
            Box::new(m20240101_000013_permissions::Migration),
            Box::new(m20240101_000014_sessions_rls::Migration),
            Box::new(m20240101_000015_rls_app_role::Migration),
            Box::new(m20240101_000016_user_lifecycle::Migration),
            Box::new(m20240101_000017_tenant_settings::Migration),
            Box::new(m20240101_000018_client_roles::Migration),
            Box::new(m20240101_000019_client_token_ttl::Migration),
            Box::new(m20240101_000020_identity_providers::Migration),
            Box::new(m20240101_000021_totp_secrets::Migration),
            Box::new(m20240101_000022_rename_role::Migration),
            Box::new(m20240101_000023_tenant_smtp_config::Migration),
            Box::new(m20240101_000024_webauthn_credentials::Migration),
        ]
    }
}
