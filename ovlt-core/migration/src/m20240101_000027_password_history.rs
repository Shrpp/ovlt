use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240101_000027_password_history"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS password_history (
                    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
                    tenant_id     UUID        NOT NULL,
                    user_id       UUID        NOT NULL,
                    password_hash TEXT        NOT NULL,
                    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
                );
                CREATE INDEX IF NOT EXISTS idx_password_history_user
                    ON password_history(tenant_id, user_id, created_at DESC);
                ALTER TABLE password_history ENABLE ROW LEVEL SECURITY;
                ALTER TABLE password_history FORCE ROW LEVEL SECURITY;
                CREATE POLICY tenant_isolation ON password_history
                    USING (tenant_id = current_setting('app.tenant_id', true)::uuid);
                GRANT SELECT, INSERT, DELETE ON password_history TO ovlt_rls;",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS password_history;")
            .await?;
        Ok(())
    }
}
