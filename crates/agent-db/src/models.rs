//! Database row models mapped to SQLite tables.
//!
//! Each struct here represents a single row in its corresponding SQLite
//! table. All models derive `sqlx::FromRow` for automatic mapping from
//! query result sets, as well as `Serialize`/`Deserialize` for JSON
//! serialization in API responses and `Debug`/`Clone` for convenience.
//!
//! Timestamps (e.g. `created_at`, `updated_at`) are stored as ISO 8601
//! strings in UTC.

use serde::{Deserialize, Serialize};

/// A key-value pair in the `config` table.
///
/// Configuration is organized by `category` (e.g. `llm`, `smtp`, `system`)
/// and includes a human-readable `description` for the management UI.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConfigRow {
    pub key: String,
    pub value: String,
    pub category: String,
    pub description: String,
    pub updated_at: String,
}

/// A conversation session in the `sessions` table.
///
/// Each session represents an independent conversation thread with its
/// own system prompt, model settings, and optional channel binding
/// (e.g. a specific Feishu group chat).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    pub name: String,
    pub agent_id: Option<String>,
    pub system_prompt: Option<String>,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: i64,
    pub channel: String,
    pub channel_chat_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A chat message in the `messages` table within a session.
///
/// Messages track the conversation role (`user`, `assistant`, `tool`, `system`)
/// and may include tool-call payloads when the LLM requests tool invocations.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MessageRow {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    /// JSON-encoded tool call requests from the LLM.
    pub tool_calls: Option<String>,
    /// The tool call ID this message responds to (for `tool` role messages).
    pub tool_call_id: Option<String>,
    pub created_at: String,
}

/// A platform channel configuration in the `channels` table.
///
/// Channels represent external integrations (Feishu, QQ, WeChat Work,
/// Webhook). The `config` field stores channel-type-specific settings
/// as a JSON string.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChannelRow {
    pub id: String,
    pub channel_type: String,
    pub name: String,
    pub enabled: bool,
    /// Channel-type-specific JSON configuration (credentials, URLs, etc.).
    pub config: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A workflow definition stored in the `workflows` table.
///
/// The `definition` field contains the full DAG (steps, edges, positions)
/// serialized as a JSON string from [`agent_core::workflow::types::WorkflowDefinition`].
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkflowRow {
    pub id: String,
    pub name: String,
    pub description: String,
    /// JSON-encoded [`WorkflowDefinition`](agent_core::workflow::types::WorkflowDefinition).
    pub definition: String,
    pub trigger_type: String,
    pub cron_expression: Option<String>,
    pub enabled: bool,
    pub last_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A workflow execution record in the `workflow_runs` table.
///
/// Created each time a workflow is triggered (manually or by cron).
/// The `result` field stores the serialized [`WorkflowResult`](agent_core::workflow::types::WorkflowResult).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkflowRunRow {
    pub id: String,
    pub workflow_id: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    /// JSON-encoded [`WorkflowResult`](agent_core::workflow::types::WorkflowResult).
    pub result: Option<String>,
    /// URL path for the published result page, if publishing was enabled.
    pub publish_url: Option<String>,
}

/// A scheduled task in the `scheduled_tasks` table.
///
/// Tasks run on a cron schedule and send a predefined prompt to the LLM,
/// optionally within a specific session context.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScheduledTaskRow {
    pub id: String,
    pub name: String,
    /// Cron expression (e.g. `0 9 * * *` for 9 AM daily).
    pub cron_expression: String,
    /// The prompt text sent to the LLM on each execution.
    pub prompt: String,
    /// Optional session to associate task execution with.
    pub session_id: Option<String>,
    pub model: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// A task execution log in the `task_logs` table.
///
/// One record is created per task execution, recording the status
/// (`success` or `error`), output, and timing.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskLogRow {
    pub id: String,
    pub task_id: String,
    pub status: String,
    pub output: Option<String>,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}
