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
                CREATE TABLE IF NOT EXISTS oauth_accounts (
                    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
                    tenant_id        UUID        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                    user_id          UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    provider         TEXT        NOT NULL,
                    provider_user_id TEXT        NOT NULL,
                    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
                    UNIQUE(tenant_id, provider, provider_user_id)
                );

                ALTER TABLE oauth_accounts ENABLE ROW LEVEL SECURITY;

                CREATE POLICY tenant_isolation ON oauth_accounts
                    USING (tenant_id = current_setting('app.tenant_id', true)::uuid);

                CREATE INDEX idx_oauth_accounts_tenant ON oauth_accounts(tenant_id, provider, provider_user_id);
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS oauth_accounts;")
            .await?;
        Ok(())
    }
}
