use sqlx::SqlitePool;

use crate::error::DbError;
use crate::models::ChannelRow;

/// Repository for platform channel configurations in the `channels` table.
///
/// Manages external platform integrations (Feishu, QQ, WeChat Work, Webhook).
/// Channel configuration is stored as JSON in the `config` column and
/// deserialized per channel type.
#[derive(Clone)]
pub struct ChannelRepo {
    pool: SqlitePool,
}

impl ChannelRepo {
    /// Create a new `ChannelRepo` backed by the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<ChannelRow>, DbError> {
        let rows = sqlx::query_as::<_, ChannelRow>(
            "SELECT * FROM channels ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_enabled(&self) -> Result<Vec<ChannelRow>, DbError> {
        let rows = sqlx::query_as::<_, ChannelRow>(
            "SELECT * FROM channels WHERE enabled = 1 ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: &str) -> Result<Option<ChannelRow>, DbError> {
        let row = sqlx::query_as::<_, ChannelRow>("SELECT * FROM channels WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn get_by_type(&self, channel_type: &str) -> Result<Option<ChannelRow>, DbError> {
        let row = sqlx::query_as::<_, ChannelRow>(
            "SELECT * FROM channels WHERE channel_type = ? LIMIT 1",
        )
        .bind(channel_type)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn create(&self, channel: &ChannelRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO channels (id, channel_type, name, enabled, config)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&channel.id)
        .bind(&channel.channel_type)
        .bind(&channel.name)
        .bind(channel.enabled)
        .bind(&channel.config)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update(&self, channel: &ChannelRow) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE channels SET name=?, enabled=?, config=?, updated_at=datetime('now')
             WHERE id=?",
        )
        .bind(&channel.name)
        .bind(channel.enabled)
        .bind(&channel.config)
        .bind(&channel.id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM channels WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
