use sqlx::SqlitePool;

use crate::error::DbError;
use crate::models::{ScheduledTaskRow, TaskLogRow};

/// Repository for scheduled tasks and execution logs.
///
/// Manages `scheduled_tasks` (cron-based automated prompts) and their
/// `task_logs` (execution records with status, output, and timing).
/// Tasks can be enabled/disabled, triggered manually, and their execution
/// history queried.
#[derive(Clone)]
pub struct TaskRepo {
    pool: SqlitePool,
}

impl TaskRepo {
    /// Create a new `TaskRepo` backed by the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ── Scheduled tasks ──

    pub async fn list(&self) -> Result<Vec<ScheduledTaskRow>, DbError> {
        let rows = sqlx::query_as::<_, ScheduledTaskRow>(
            "SELECT * FROM scheduled_tasks ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_enabled(&self) -> Result<Vec<ScheduledTaskRow>, DbError> {
        let rows = sqlx::query_as::<_, ScheduledTaskRow>(
            "SELECT * FROM scheduled_tasks WHERE enabled = 1",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: &str) -> Result<Option<ScheduledTaskRow>, DbError> {
        let row =
            sqlx::query_as::<_, ScheduledTaskRow>("SELECT * FROM scheduled_tasks WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    pub async fn create(&self, task: &ScheduledTaskRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO scheduled_tasks (id, name, cron_expression, prompt, session_id, model, enabled)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&task.id)
        .bind(&task.name)
        .bind(&task.cron_expression)
        .bind(&task.prompt)
        .bind(&task.session_id)
        .bind(&task.model)
        .bind(task.enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update(&self, task: &ScheduledTaskRow) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE scheduled_tasks SET name=?, cron_expression=?, prompt=?, session_id=?,
             model=?, enabled=?, updated_at=datetime('now') WHERE id=?",
        )
        .bind(&task.name)
        .bind(&task.cron_expression)
        .bind(&task.prompt)
        .bind(&task.session_id)
        .bind(&task.model)
        .bind(task.enabled)
        .bind(&task.id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM scheduled_tasks WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Task execution logs ──

    pub async fn create_log(&self, log: &TaskLogRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO task_logs (id, task_id, status, output, error)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&log.id)
        .bind(&log.task_id)
        .bind(&log.status)
        .bind(&log.output)
        .bind(&log.error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_log(&self, log: &TaskLogRow) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE task_logs SET status=?, output=?, error=?, finished_at=datetime('now')
             WHERE id=?",
        )
        .bind(&log.status)
        .bind(&log.output)
        .bind(&log.error)
        .bind(&log.id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_logs(
        &self,
        task_id: &str,
        limit: i64,
    ) -> Result<Vec<TaskLogRow>, DbError> {
        let rows = sqlx::query_as::<_, TaskLogRow>(
            "SELECT * FROM task_logs WHERE task_id = ? ORDER BY started_at DESC LIMIT ?",
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
