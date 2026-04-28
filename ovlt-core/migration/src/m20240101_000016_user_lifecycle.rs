use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240101_000016_user_lifecycle"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        // Track whether email has been verified.
        conn.execute_unprepared(
            "ALTER TABLE users ADD COLUMN IF NOT EXISTS email_verified BOOLEAN NOT NULL DEFAULT false",
        )
        .await?;

        // One-time tokens for password reset and email verification.
        conn.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS one_time_tokens (
                id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id   UUID        NOT NULL,
                user_id     UUID        NOT NULL,
                token_hash  TEXT        NOT NULL UNIQUE,
                token_type  TEXT        NOT NULL,
                expires_at  TIMESTAMPTZ NOT NULL,
                used_at     TIMESTAMPTZ
            )",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_ott_token ON one_time_tokens (token_hash)",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_ott_user ON one_time_tokens (user_id, tenant_id, token_type)",
        )
        .await?;

        // Per-tenant password policy.
        conn.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS password_policies (
                tenant_id         UUID    PRIMARY KEY,
                min_length        INTEGER NOT NULL DEFAULT 8,
                require_uppercase BOOLEAN NOT NULL DEFAULT false,
                require_digit     BOOLEAN NOT NULL DEFAULT false,
                require_special   BOOLEAN NOT NULL DEFAULT false,
                history_size      INTEGER NOT NULL DEFAULT 0,
                updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared("DROP TABLE IF EXISTS password_policies")
            .await?;
        conn.execute_unprepared("DROP TABLE IF EXISTS one_time_tokens")
            .await?;
        conn.execute_unprepared("ALTER TABLE users DROP COLUMN IF EXISTS email_verified")
            .await?;
        Ok(())
    }
}
