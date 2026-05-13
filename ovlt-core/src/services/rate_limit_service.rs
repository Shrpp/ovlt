use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};

use crate::error::AppError;

pub const WINDOW_SECS: i64 = 60;
pub const MAX_REQUESTS: i32 = 20;

/// Atomically increments the request counter for `key` in the current fixed window.
/// Returns `true` if the request is allowed, `false` if the limit is exceeded.
///
/// Uses a single `INSERT ... ON CONFLICT DO UPDATE RETURNING count` — safe across
/// multiple replicas sharing the same PostgreSQL instance.
pub async fn check_and_increment(db: &DatabaseConnection, key: &str) -> Result<bool, AppError> {
    let now = Utc::now();
    let window_start: i64 = now.timestamp() / WINDOW_SECS;
    let expires_at = (now + chrono::Duration::seconds(WINDOW_SECS * 2)).fixed_offset();

    let row = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"INSERT INTO rate_limit_buckets (key, window_start, count, expires_at)
               VALUES ($1, $2, 1, $3)
               ON CONFLICT (key, window_start)
               DO UPDATE SET count = rate_limit_buckets.count + 1
               RETURNING count"#,
            [key.into(), window_start.into(), expires_at.into()],
        ))
        .await?;

    let count: i32 = row
        .and_then(|r| r.try_get::<i32>("", "count").ok())
        .unwrap_or(1);

    Ok(count <= MAX_REQUESTS)
}

pub async fn cleanup_expired(db: &DatabaseConnection) -> Result<u64, AppError> {
    let result = db
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "DELETE FROM rate_limit_buckets WHERE expires_at < NOW()".to_owned(),
        ))
        .await?;
    Ok(result.rows_affected())
}
