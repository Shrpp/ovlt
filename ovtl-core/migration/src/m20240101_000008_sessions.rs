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
                CREATE TABLE IF NOT EXISTS sessions (
                    id          TEXT        PRIMARY KEY,
                    tenant_id   UUID        NOT NULL,
                    user_id     UUID        NOT NULL,
                    data        JSONB       NOT NULL DEFAULT '{}'::jsonb,
                    expires_at  TIMESTAMPTZ NOT NULL,
                    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
                );
                CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions (expires_at);
                CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions (tenant_id, user_id);
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS sessions")
            .await?;
        Ok(())
    }
}
