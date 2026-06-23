//! Workflow engine for DAG-based multi-step task execution.
//!
//! Workflows are defined as a Directed Acyclic Graph (DAG) of steps connected
//! by edges. The [`WorkflowEngine`] performs topological sort and executes
//! steps in parallel batches. Supported step types include LLM calls, tool
//! invocations, publish actions, conditions, and delays.
//!
//! See [`WorkflowDefinition`] for the schema and [`WorkflowEngine::execute`]
//! for the execution entry point.

pub mod engine;
pub mod types;

pub use engine::WorkflowEngine;
pub use types::*;
