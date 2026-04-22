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
                CREATE TABLE IF NOT EXISTS refresh_tokens (
                    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
                    tenant_id   UUID        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    token_hash  TEXT        NOT NULL,
                    expires_at  TIMESTAMPTZ NOT NULL,
                    revoked_at  TIMESTAMPTZ,
                    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
                );

                ALTER TABLE refresh_tokens ENABLE ROW LEVEL SECURITY;

                CREATE POLICY tenant_isolation ON refresh_tokens
                    USING (tenant_id = current_setting('app.tenant_id', true)::uuid);

                CREATE INDEX idx_refresh_tokens_tenant_user ON refresh_tokens(tenant_id, user_id);
                CREATE INDEX idx_refresh_tokens_hash        ON refresh_tokens(token_hash);
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS refresh_tokens;")
            .await?;
        Ok(())
    }
}
