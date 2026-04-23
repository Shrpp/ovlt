use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE IF NOT EXISTS oauth_clients (
                    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
                    tenant_id       UUID        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                    client_id       TEXT        NOT NULL UNIQUE,
                    client_secret   TEXT        NOT NULL,
                    name            TEXT        NOT NULL,
                    redirect_uris   JSONB       NOT NULL DEFAULT '[]'::jsonb,
                    grant_types     JSONB       NOT NULL DEFAULT '["authorization_code"]'::jsonb,
                    scopes          JSONB       NOT NULL DEFAULT '["openid","email","profile"]'::jsonb,
                    is_confidential BOOLEAN     NOT NULL DEFAULT true,
                    require_consent BOOLEAN     NOT NULL DEFAULT false,
                    is_active       BOOLEAN     NOT NULL DEFAULT true,
                    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
                );
                CREATE INDEX IF NOT EXISTS idx_oauth_clients_tenant ON oauth_clients (tenant_id);
                ALTER TABLE oauth_clients ENABLE ROW LEVEL SECURITY;
                ALTER TABLE oauth_clients FORCE ROW LEVEL SECURITY;
                CREATE POLICY tenant_isolation ON oauth_clients
                    USING (tenant_id = current_setting('app.tenant_id')::UUID);
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS oauth_clients CASCADE")
            .await?;
        Ok(())
    }
}
