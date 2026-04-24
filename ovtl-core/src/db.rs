use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbErr,
    TransactionTrait,
};
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

/// Opens a transaction, switches to the non-superuser `ovtl_rls` role, and sets
/// `app.tenant_id` so PostgreSQL RLS policies activate.
/// The superuser session bypasses RLS; switching role drops that privilege for the
/// duration of this transaction so FORCE ROW LEVEL SECURITY actually fires.
pub async fn begin_tenant_txn(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<DatabaseTransaction, DbErr> {
    let txn = db.begin().await?;
    txn.execute_unprepared("SET LOCAL ROLE ovtl_rls").await?;
    txn.execute_unprepared(&format!("SET LOCAL app.tenant_id = '{tenant_id}'"))
        .await?;
    Ok(txn)
}

pub async fn connect(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut opts = ConnectOptions::new(database_url.to_owned());
    opts.max_connections(20)
        .min_connections(2)
        .connect_timeout(Duration::from_secs(10))
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        .sqlx_logging(false);

    let db = Database::connect(opts).await?;
    info!("Connected to PostgreSQL ✓");
    Ok(db)
}
