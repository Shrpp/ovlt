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
                CREATE TABLE IF NOT EXISTS tenants (
                    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
                    name            TEXT        NOT NULL,
                    slug            TEXT        NOT NULL UNIQUE,
                    encryption_key  TEXT        NOT NULL,
                    plan            TEXT        NOT NULL DEFAULT 'starter',
                    is_active       BOOLEAN     NOT NULL DEFAULT true,
                    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
                );
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS tenants;")
            .await?;
        Ok(())
    }
}
