use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240101_000023_tenant_smtp_config"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS tenant_smtp_config (
                    tenant_id    UUID        PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
                    host         TEXT        NOT NULL,
                    port         INTEGER     NOT NULL DEFAULT 587,
                    username     TEXT        NOT NULL,
                    password_enc TEXT        NOT NULL,
                    from_name    TEXT        NOT NULL,
                    from_email   TEXT        NOT NULL,
                    use_tls      BOOLEAN     NOT NULL DEFAULT true,
                    enabled      BOOLEAN     NOT NULL DEFAULT false,
                    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
                )",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS tenant_smtp_config")
            .await?;
        Ok(())
    }
}
