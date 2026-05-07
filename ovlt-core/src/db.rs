use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, DbErr, Statement, TransactionTrait,
};
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

/// Opens a transaction, switches to the non-superuser `ovlt_rls` role, and sets
/// `app.tenant_id` so PostgreSQL RLS policies activate.
/// The superuser session bypasses RLS; switching role drops that privilege for the
/// duration of this transaction so FORCE ROW LEVEL SECURITY actually fires.
pub async fn begin_tenant_txn(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<DatabaseTransaction, DbErr> {
    let txn = db.begin().await?;
    txn.execute_unprepared("SET LOCAL ROLE ovlt_rls").await?;
    txn.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT set_config('app.tenant_id', $1, true)",
        [tenant_id.to_string().into()],
    ))
    .await?;
    Ok(txn)
}

pub async fn connect(
    database_url: &str,
    max_connections: u32,
    min_connections: u32,
) -> Result<DatabaseConnection, DbErr> {
    let mut opts = ConnectOptions::new(database_url.to_owned());
    opts.max_connections(max_connections)
        .min_connections(min_connections)
        .connect_timeout(Duration::from_secs(10))
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        .sqlx_logging(false);

    let db = Database::connect(opts).await?;
    info!("Connected to PostgreSQL ✓");
    Ok(db)
}
