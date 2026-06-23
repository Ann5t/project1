//! Connection pool management for the SQLite database.
//!
//! Creates a connection pool with conservative settings suitable for
//! single-server deployments. SQLite only supports a single writer at a
//! time, so the pool is sized accordingly.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::time::Duration;

/// Create a new SQLite connection pool.
///
/// # Pool Configuration
///
/// - **Max connections**: 5 (SQLite has single-writer limitation)
/// - **Acquire timeout**: 5 seconds (fail fast on pool exhaustion)
///
/// # Arguments
///
/// * `database_url` - SQLite connection string, e.g. `sqlite:data/agent.db?mode=rwc`
///
/// # Errors
///
/// Returns [`DbError::Connection`] if the database file cannot be opened or
/// the pool cannot be created.
pub async fn create_pool(database_url: &str) -> Result<SqlitePool, crate::error::DbError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
        .map_err(|e| crate::error::DbError::Connection(e.to_string()))?;

    Ok(pool)
}
