use sqlx::SqlitePool;
use std::collections::HashMap;

use crate::error::DbError;

/// Repository for key-value application configuration stored in the `config` table.
///
/// Supports reading individual keys, batch updates, categorized grouping,
/// and deletion. Uses SQLite `INSERT ... ON CONFLICT DO UPDATE` (upsert)
/// semantics so setting a key that already exists silently updates it.
///
/// # Example
///
/// ```no_run
/// # use agent_db::repo::ConfigRepo;
/// # async fn example(pool: sqlx::SqlitePool) -> Result<(), agent_db::error::DbError> {
/// let repo = ConfigRepo::new(pool);
///
/// // Set a value
/// repo.set("api_key", "sk-xxxx").await?;
///
/// // Get a value
/// let api_key = repo.get("api_key").await?;
/// assert_eq!(api_key, Some("sk-xxxx".to_string()));
///
/// // Get all as HashMap
/// let all = repo.get_all().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ConfigRepo {
    pool: SqlitePool,
}

impl ConfigRepo {
    /// Create a new `ConfigRepo` backed by the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get a single config value by key
    pub async fn get(&self, key: &str) -> Result<Option<String>, DbError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM config WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Get config value with default fallback
    pub async fn get_or_default(&self, key: &str, default: &str) -> Result<String, DbError> {
        Ok(self.get(key).await?.unwrap_or_else(|| default.to_string()))
    }

    /// Set a single config value (insert or update)
    pub async fn set(&self, key: &str, value: &str) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO config (key, value, updated_at) VALUES (?, ?, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all config values as a `HashMap`
    pub async fn get_all(&self) -> Result<HashMap<String, String>, DbError> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM config ORDER BY category, key")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().collect())
    }

    /// Get all config grouped by category
    pub async fn get_all_grouped(&self) -> Result<HashMap<String, HashMap<String, String>>, DbError> {
        let rows: Vec<(String, String, String)> =
            sqlx::query_as("SELECT key, value, category FROM config ORDER BY category, key")
                .fetch_all(&self.pool)
                .await?;

        let mut grouped: HashMap<String, HashMap<String, String>> = HashMap::new();
        for (key, value, category) in rows {
            grouped.entry(category).or_default().insert(key, value);
        }
        Ok(grouped)
    }

    /// Batch update config values
    pub async fn update_all(&self, config: &HashMap<String, String>) -> Result<(), DbError> {
        for (key, value) in config {
            self.set(key, value).await?;
        }
        Ok(())
    }

    /// Delete a config key
    pub async fn delete(&self, key: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM config WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
