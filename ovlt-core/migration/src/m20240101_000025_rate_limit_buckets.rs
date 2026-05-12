use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240101_000025_rate_limit_buckets"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS rate_limit_buckets (
                    key          TEXT        NOT NULL,
                    window_start BIGINT      NOT NULL,
                    count        INTEGER     NOT NULL DEFAULT 1,
                    expires_at   TIMESTAMPTZ NOT NULL,
                    PRIMARY KEY (key, window_start)
                );
                CREATE INDEX IF NOT EXISTS idx_rate_limit_buckets_expires_at
                    ON rate_limit_buckets (expires_at);",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS rate_limit_buckets")
            .await?;
        Ok(())
    }
}
