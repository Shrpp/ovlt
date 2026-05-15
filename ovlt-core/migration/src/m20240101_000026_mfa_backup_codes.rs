use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS mfa_backup_codes (
                    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
                    tenant_id   UUID        NOT NULL,
                    user_id     UUID        NOT NULL,
                    code_hash   TEXT        NOT NULL,
                    used_at     TIMESTAMPTZ,
                    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
                );
                CREATE INDEX IF NOT EXISTS idx_mfa_backup_codes_user
                    ON mfa_backup_codes(tenant_id, user_id);
                ALTER TABLE mfa_backup_codes ENABLE ROW LEVEL SECURITY;
                ALTER TABLE mfa_backup_codes FORCE ROW LEVEL SECURITY;
                CREATE POLICY tenant_isolation ON mfa_backup_codes
                    USING (tenant_id = current_setting('app.tenant_id', true)::uuid);
                GRANT SELECT, INSERT, UPDATE, DELETE ON mfa_backup_codes TO ovlt_rls;",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS mfa_backup_codes;")
            .await?;
        Ok(())
    }
}
