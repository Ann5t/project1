//! Unified error type for the agent-core crate.
//!
//! Every fallible operation in the core crate returns `Result<T, CoreError>`.
//! Variants cover LLM API failures, rate limiting, tool errors, workflow
//! failures, and data serialization issues.

use std::time::Duration;
use thiserror::Error;

/// Unified error type for all core operations.
///
/// Implements `thiserror::Error` for display formatting and automatic
/// `From` conversions for `serde_json::Error`, `reqwest::Error`, and
/// `agent_db::error::DbError`.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("LLM API error: {0}")]
    LlmApi(String),

    #[error("LLM rate limited, retry after {retry_after:?}")]
    RateLimited { retry_after: Option<Duration> },

    #[error("Tool execution failed: {tool} — {message}")]
    ToolError { tool: String, message: String },

    #[error("Scheduler error: {0}")]
    Scheduler(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Workflow not found: {0}")]
    WorkflowNotFound(String),

    #[error("Workflow execution error: {0}")]
    WorkflowExecution(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Database error: {0}")]
    Db(#[from] agent_db::error::DbError),

    #[error("Channel error: {0}")]
    Channel(String),
}
