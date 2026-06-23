use sqlx::SqlitePool;

use crate::error::DbError;
use crate::models::MessageRow;

/// Repository for chat messages in the `messages` table.
///
/// Messages are always scoped to a session. Supports listing messages
/// in chronological order, inserting new messages, and bulk deletion
/// when a session is removed.
#[derive(Clone)]
pub struct MessageRepo {
    pool: SqlitePool,
}

impl MessageRepo {
    /// Create a new `MessageRepo` backed by the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get all messages for a session, ordered by creation time
    pub async fn list_by_session(&self, session_id: &str) -> Result<Vec<MessageRow>, DbError> {
        let rows = sqlx::query_as::<_, MessageRow>(
            "SELECT * FROM messages WHERE session_id = ? ORDER BY created_at ASC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get recent messages for a session (limited count, for context window)
    pub async fn list_recent(
        &self,
        session_id: &str,
        limit: i64,
    ) -> Result<Vec<MessageRow>, DbError> {
        let rows = sqlx::query_as::<_, MessageRow>(
            "SELECT * FROM messages WHERE session_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Insert a message
    pub async fn insert(&self, msg: &MessageRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, tool_calls, tool_call_id)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&msg.id)
        .bind(&msg.session_id)
        .bind(&msg.role)
        .bind(&msg.content)
        .bind(&msg.tool_calls)
        .bind(&msg.tool_call_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete all messages for a session
    pub async fn delete_by_session(&self, session_id: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM messages WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
