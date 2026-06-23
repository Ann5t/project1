use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// A tool that can be called by the LLM
#[async_trait]
pub trait Tool: Send + Sync {
    /// Return the tool name
    fn name(&self) -> &str;

    /// Return the tool description for the LLM
    fn description(&self) -> &str;

    /// Return the JSON Schema for tool parameters
    fn parameters_schema(&self) -> serde_json::Value;

    /// Execute the tool with the given arguments
    async fn execute(&self, arguments: serde_json::Value) -> Result<String, CoreError>;
}

/// Tool call request (what the LLM sends)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub content: String,
    pub success: bool,
}
