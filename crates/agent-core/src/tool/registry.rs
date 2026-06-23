use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::types::{Tool, ToolCallResult};
use crate::error::CoreError;

/// Registry of available tools for LLM function calling.
///
/// Tools are stored as `Arc<dyn Tool>` and accessed by name. The registry
/// is interior-mutable via `tokio::sync::RwLock`, making it safe for
/// concurrent registration and execution. Use [`ToolRegistry::default`]
/// or [`ToolRegistry::new`] to create an empty registry.
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
}

impl ToolRegistry {
    /// Create an empty tool registry.
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new tool by name. If a tool with the same name already
    /// exists, it is replaced.
    pub async fn register(&self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        info!("Registering tool: {}", name);
        self.tools.write().await.insert(name, tool);
    }

    /// Unregister a tool by name. No-op if the tool does not exist.
    pub async fn unregister(&self, name: &str) {
        self.tools.write().await.remove(name);
    }

    /// Look up a tool by name. Returns `None` if not registered.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.read().await.get(name).cloned()
    }

    /// Return all registered tool definitions in the format expected by the
    /// LLM's `tools` parameter (OpenAI function-calling schema).
    pub async fn get_definitions(&self) -> Vec<serde_json::Value> {
        self.tools
            .read()
            .await
            .values()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.parameters_schema(),
                    }
                })
            })
            .collect()
    }

    /// Execute a tool by name with the given JSON arguments.
    ///
    /// Returns a [`ToolCallResult`] on success or a [`CoreError::ToolError`]
    /// if the tool is not found or fails during execution.
    pub async fn execute(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolCallResult, CoreError> {
        let tool = self.get(name).await.ok_or_else(|| CoreError::ToolError {
            tool: name.to_string(),
            message: "Tool not found".into(),
        })?;

        debug!("Executing tool: {} with args: {}", name, arguments);

        match tool.execute(arguments).await {
            Ok(content) => Ok(ToolCallResult {
                id: String::new(),
                name: name.to_string(),
                content,
                success: true,
            }),
            Err(e) => {
                warn!("Tool {} execution failed: {}", name, e);
                Err(e)
            }
        }
    }

    /// Return the names of all registered tools.
    pub async fn list_names(&self) -> Vec<String> {
        self.tools.read().await.keys().cloned().collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
