use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbErr,
    TransactionTrait,
};
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

/// Opens a transaction and sets `app.tenant_id` so PostgreSQL RLS policies activate.
/// All DB operations inside the returned transaction are automatically tenant-scoped.
pub async fn begin_tenant_txn(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<DatabaseTransaction, DbErr> {
    let txn = db.begin().await?;
    txn.execute_unprepared(&format!(
        "SET LOCAL app.tenant_id = '{tenant_id}'"
    ))
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
