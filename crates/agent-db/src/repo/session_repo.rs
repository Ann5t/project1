use sqlx::SqlitePool;

use crate::error::DbError;
use crate::models::SessionRow;

/// Repository for conversation sessions in the `sessions` table.
///
/// Provides CRUD operations for session management, including listing
/// all sessions (newest first), retrieving by ID, creating, updating,
/// and deleting (with cascade to messages).
#[derive(Clone)]
pub struct SessionRepo {
    pool: SqlitePool,
}

impl SessionRepo {
    /// Create a new `SessionRepo` backed by the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// List all sessions, newest first
    pub async fn list(&self) -> Result<Vec<SessionRow>, DbError> {
        let rows =
            sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions ORDER BY updated_at DESC")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    /// Get a single session by id
    pub async fn get(&self, id: &str) -> Result<Option<SessionRow>, DbError> {
        let row = sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Create a new session
    pub async fn create(&self, session: &SessionRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO sessions (id, name, agent_id, system_prompt, model, temperature, max_tokens, channel, channel_chat_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&session.id)
        .bind(&session.name)
        .bind(&session.agent_id)
        .bind(&session.system_prompt)
        .bind(&session.model)
        .bind(session.temperature)
        .bind(session.max_tokens)
        .bind(&session.channel)
        .bind(&session.channel_chat_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update a session
    pub async fn update(&self, session: &SessionRow) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE sessions SET name=?, agent_id=?, system_prompt=?, model=?, temperature=?, max_tokens=?,
             channel=?, channel_chat_id=?, updated_at=datetime('now') WHERE id=?",
        )
        .bind(&session.name)
        .bind(&session.agent_id)
        .bind(&session.system_prompt)
        .bind(&session.model)
        .bind(session.temperature)
        .bind(session.max_tokens)
        .bind(&session.channel)
        .bind(&session.channel_chat_id)
        .bind(&session.id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update session timestamp
    pub async fn touch(&self, id: &str) -> Result<(), DbError> {
        sqlx::query("UPDATE sessions SET updated_at=datetime('now') WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Delete a session (cascades to messages)
    pub async fn delete(&self, id: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
