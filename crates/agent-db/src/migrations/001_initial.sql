-- crates/agent-db/src/migrations/001_initial.sql
-- Initial database schema for AI Agent

-- Configuration as key-value store
CREATE TABLE IF NOT EXISTS config (
    key         TEXT PRIMARY KEY NOT NULL,
    value       TEXT NOT NULL,
    category    TEXT NOT NULL DEFAULT 'general',
    description TEXT NOT NULL DEFAULT '',
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Sessions (conversation sessions / agents)
CREATE TABLE IF NOT EXISTS sessions (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    agent_id        TEXT,
    system_prompt   TEXT,
    model           TEXT NOT NULL DEFAULT 'deepseek-chat',
    temperature     REAL NOT NULL DEFAULT 0.7,
    max_tokens      INTEGER NOT NULL DEFAULT 4096,
    channel         TEXT NOT NULL DEFAULT 'web',
    channel_chat_id TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Messages within a session
CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY NOT NULL,
    session_id      TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role            TEXT NOT NULL CHECK(role IN ('system','user','assistant','tool')),
    content         TEXT NOT NULL,
    tool_calls      TEXT,
    tool_call_id    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_messages_session
    ON messages(session_id, created_at);

-- Channel configurations (Feishu, QQ, WeChat, etc.)
CREATE TABLE IF NOT EXISTS channels (
    id           TEXT PRIMARY KEY NOT NULL,
    channel_type TEXT NOT NULL CHECK(channel_type IN ('feishu','qq','wechat_work','webhook')),
    name         TEXT NOT NULL,
    enabled      INTEGER NOT NULL DEFAULT 0,
    config       TEXT NOT NULL DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Workflow definitions (DAG)
CREATE TABLE IF NOT EXISTS workflows (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    definition      TEXT NOT NULL DEFAULT '{"steps":[],"edges":[]}',
    trigger_type    TEXT NOT NULL DEFAULT 'manual' CHECK(trigger_type IN ('manual','cron')),
    cron_expression TEXT,
    enabled         INTEGER NOT NULL DEFAULT 1,
    last_run_at     TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Workflow execution records
CREATE TABLE IF NOT EXISTS workflow_runs (
    id          TEXT PRIMARY KEY NOT NULL,
    workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    status      TEXT NOT NULL CHECK(status IN ('running','success','error','cancelled')),
    started_at  TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT,
    result      TEXT,
    publish_url TEXT
);

CREATE INDEX IF NOT EXISTS idx_workflow_runs_workflow
    ON workflow_runs(workflow_id, started_at DESC);

-- Scheduled tasks (simple single-step cron tasks)
CREATE TABLE IF NOT EXISTS scheduled_tasks (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    cron_expression TEXT NOT NULL,
    prompt          TEXT NOT NULL,
    session_id      TEXT,
    model           TEXT NOT NULL DEFAULT 'deepseek-chat',
    enabled         INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Task execution logs
CREATE TABLE IF NOT EXISTS task_logs (
    id          TEXT PRIMARY KEY NOT NULL,
    task_id     TEXT NOT NULL REFERENCES scheduled_tasks(id) ON DELETE CASCADE,
    status      TEXT NOT NULL CHECK(status IN ('running','success','error')),
    output      TEXT,
    error       TEXT,
    started_at  TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_task_logs_task
    ON task_logs(task_id, started_at DESC);
