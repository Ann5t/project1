//! # agent-core
//!
//! Core logic crate for the AI Agent application.
//!
//! This crate provides:
//!
//! - **llm**: LLM client abstraction (`LlmClient` trait) with a DeepSeek API
//!   implementation supporting both streaming and non-streaming chat completions.
//! - **tool**: Tool system with a `Tool` trait, `ToolRegistry` for registration
//!   and discovery, and built-in tools (calculator, web search, file read, etc.).
//! - **session**: `SessionManager` that orchestrates the agent loop -- loading
//!   conversation history, calling the LLM, executing tool calls, and streaming
//!   responses back via SSE-style events.
//! - **workflow**: `WorkflowEngine` that executes a DAG (Directed Acyclic Graph)
//!   of steps with topological sort and parallel batch execution.
//! - **scheduler**: `TaskSchedulerEngine` for cron-based scheduled task execution.
//! - **error**: Unified `CoreError` enum covering all failure modes.

// agent-core: pedantic lints for core library — deny pedantic issues
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::doc_markdown)]

pub mod error;
pub mod llm;
pub mod scheduler;
pub mod session;
pub mod tool;
pub mod workflow;

pub use error::CoreError;
