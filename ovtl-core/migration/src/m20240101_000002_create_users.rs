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
                CREATE TABLE IF NOT EXISTS users (
                    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
                    tenant_id     UUID        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                    email         TEXT        NOT NULL,
                    email_lookup  TEXT        NOT NULL,
                    password_hash TEXT        NOT NULL,
                    is_active     BOOLEAN     NOT NULL DEFAULT true,
                    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
                    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
                    UNIQUE(tenant_id, email_lookup)
                );

                ALTER TABLE users ENABLE ROW LEVEL SECURITY;

                CREATE POLICY tenant_isolation ON users
                    USING (tenant_id = current_setting('app.tenant_id', true)::uuid);

                CREATE INDEX idx_users_tenant_id    ON users(tenant_id);
                CREATE INDEX idx_users_email_lookup ON users(tenant_id, email_lookup);
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS users;")
            .await?;
        Ok(())
    }
}
