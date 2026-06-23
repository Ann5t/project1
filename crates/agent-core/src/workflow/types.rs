use serde::{Deserialize, Serialize};

/// A workflow definition (DAG)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub steps: Vec<WorkflowStep>,
    pub edges: Vec<WorkflowEdge>,
}

/// A single step (node) in a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub step_type: StepType,
    #[serde(default)]
    pub config: serde_json::Value,
    /// Position for the visual editor
    #[serde(default)]
    pub position: Option<StepPosition>,
}

/// Type of workflow step
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    #[default]
    LlmCall,
    ToolCall,
    Publish,
    Condition,
    Delay,
}

/// Position for graphical editor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepPosition {
    pub x: f64,
    pub y: f64,
}

/// An edge (connection) between steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
}

/// Execution status of a workflow step
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Success,
    Error,
    Skipped,
}

/// Result of running a workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub status: StepStatus,
    pub output: Option<String>,
    pub error: Option<String>,
}

/// Overall workflow execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub workflow_id: String,
    pub run_id: String,
    pub status: StepStatus,
    pub steps: Vec<StepResult>,
    pub publish_url: Option<String>,
}
