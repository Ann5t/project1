use sqlx::SqlitePool;

use crate::error::DbError;
use crate::models::{WorkflowRow, WorkflowRunRow};

/// Repository for workflow definitions and execution history.
///
/// Manages both `workflows` (DAG definitions with steps and edges) and
/// `workflow_runs` (execution records with status and results).
/// Workflows can be triggered manually or via cron, with each execution
/// producing a run record.
#[derive(Clone)]
pub struct WorkflowRepo {
    pool: SqlitePool,
}

impl WorkflowRepo {
    /// Create a new `WorkflowRepo` backed by the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ── Workflow definitions ──

    pub async fn list(&self) -> Result<Vec<WorkflowRow>, DbError> {
        let rows =
            sqlx::query_as::<_, WorkflowRow>("SELECT * FROM workflows ORDER BY updated_at DESC")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    pub async fn list_enabled(&self) -> Result<Vec<WorkflowRow>, DbError> {
        let rows = sqlx::query_as::<_, WorkflowRow>(
            "SELECT * FROM workflows WHERE enabled = 1 ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: &str) -> Result<Option<WorkflowRow>, DbError> {
        let row = sqlx::query_as::<_, WorkflowRow>("SELECT * FROM workflows WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn create(&self, wf: &WorkflowRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO workflows (id, name, description, definition, trigger_type, cron_expression, enabled)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&wf.id)
        .bind(&wf.name)
        .bind(&wf.description)
        .bind(&wf.definition)
        .bind(&wf.trigger_type)
        .bind(&wf.cron_expression)
        .bind(wf.enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update(&self, wf: &WorkflowRow) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE workflows SET name=?, description=?, definition=?, trigger_type=?,
             cron_expression=?, enabled=?, updated_at=datetime('now') WHERE id=?",
        )
        .bind(&wf.name)
        .bind(&wf.description)
        .bind(&wf.definition)
        .bind(&wf.trigger_type)
        .bind(&wf.cron_expression)
        .bind(wf.enabled)
        .bind(&wf.id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn touch_run(&self, id: &str) -> Result<(), DbError> {
        sqlx::query("UPDATE workflows SET last_run_at=datetime('now') WHERE id=?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM workflows WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Workflow execution runs ──

    pub async fn create_run(&self, run: &WorkflowRunRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO workflow_runs (id, workflow_id, status, result, publish_url)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&run.id)
        .bind(&run.workflow_id)
        .bind(&run.status)
        .bind(&run.result)
        .bind(&run.publish_url)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_run(&self, run: &WorkflowRunRow) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE workflow_runs SET status=?, finished_at=?, result=?, publish_url=?
             WHERE id=?",
        )
        .bind(&run.status)
        .bind(&run.finished_at)
        .bind(&run.result)
        .bind(&run.publish_url)
        .bind(&run.id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_runs(
        &self,
        workflow_id: &str,
        limit: i64,
    ) -> Result<Vec<WorkflowRunRow>, DbError> {
        let rows = sqlx::query_as::<_, WorkflowRunRow>(
            "SELECT * FROM workflow_runs WHERE workflow_id = ? ORDER BY started_at DESC LIMIT ?",
        )
        .bind(workflow_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_run(&self, id: &str) -> Result<Option<WorkflowRunRow>, DbError> {
        let row = sqlx::query_as::<_, WorkflowRunRow>("SELECT * FROM workflow_runs WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }
}
