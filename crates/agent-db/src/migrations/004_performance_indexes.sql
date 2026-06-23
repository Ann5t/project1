-- crates/agent-db/src/migrations/004_performance_indexes.sql
-- Add indexes for commonly queried columns to improve listing/sorting performance

-- Index for listing recent sessions (sorted by updated_at)
CREATE INDEX IF NOT EXISTS idx_sessions_updated
    ON sessions(updated_at);

-- Index for listing recent workflow runs (sorted by started_at)
CREATE INDEX IF NOT EXISTS idx_workflow_runs_started
    ON workflow_runs(started_at);

-- Index for listing recent task logs (sorted by started_at)
CREATE INDEX IF NOT EXISTS idx_task_logs_started
    ON task_logs(started_at);
