use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS webauthn_credentials (
                    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
                    tenant_id       UUID        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                    user_id         UUID        NOT NULL,
                    credential_id   TEXT        NOT NULL,
                    public_key_json TEXT        NOT NULL,
                    name            TEXT        NOT NULL DEFAULT '',
                    aaguid          TEXT,
                    sign_count      INTEGER     NOT NULL DEFAULT 0,
                    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
                    last_used_at    TIMESTAMPTZ,
                    UNIQUE (tenant_id, credential_id)
                );
                CREATE INDEX IF NOT EXISTS idx_webauthn_user ON webauthn_credentials(tenant_id, user_id);
                ALTER TABLE webauthn_credentials ENABLE ROW LEVEL SECURITY;
                ALTER TABLE webauthn_credentials FORCE ROW LEVEL SECURITY;
                CREATE POLICY tenant_isolation ON webauthn_credentials
                    USING (tenant_id = current_setting('app.tenant_id', true)::uuid);
                GRANT SELECT, INSERT, UPDATE, DELETE ON webauthn_credentials TO ovlt_rls;",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS webauthn_credentials;")
            .await?;
        Ok(())
    }
}
