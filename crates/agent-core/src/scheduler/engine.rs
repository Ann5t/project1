use std::sync::Arc;
use tracing::{info, warn};

use crate::error::CoreError;
use crate::llm::client::LlmClient;
use crate::tool::registry::ToolRegistry;

/// Scheduler engine for cron-based tasks
/// Uses tokio-cron-scheduler for cron parsing and scheduling
pub struct TaskSchedulerEngine {
    #[expect(dead_code, reason = "Reserved for future cron-based task execution (Phase 9)")]
    llm: Arc<dyn LlmClient>,
    #[expect(dead_code, reason = "Reserved for future tool-aware task execution (Phase 9)")]
    tools: Arc<ToolRegistry>,
}

impl TaskSchedulerEngine {
    pub fn new(llm: Arc<dyn LlmClient>, tools: Arc<ToolRegistry>) -> Self {
        Self { llm, tools }
    }

    /// Start the scheduler — loads enabled tasks from DB and starts cron jobs
    pub fn start(&self) -> Result<(), CoreError> {
        info!("Task scheduler engine started");
        // Full implementation in Phase 9 — loads tasks from DB, schedules cron jobs
        Ok(())
    }

    /// Stop the scheduler gracefully
    pub fn stop(&self) -> Result<(), CoreError> {
        info!("Task scheduler engine stopped");
        Ok(())
    }

    /// Execute a task immediately (for manual trigger)
    pub fn execute_task(
        &self,
        _task_id: &str,
        _prompt: &str,
        _model: &str,
    ) -> Result<String, CoreError> {
        // Placeholder — full implementation in Phase 9
        warn!("Task execution not yet implemented");
        Ok("Task execution placeholder".into())
    }
}
