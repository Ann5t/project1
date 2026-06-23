//! Tool system for AI Agent function calling.
//!
//! Tools are discrete capabilities the LLM can invoke. Each tool implements
//! the [`Tool`] trait which describes itself (name, description, JSON Schema)
//! and provides an [`execute`](Tool::execute) method.
//!
//! The `ToolRegistry` manages tool registration, discovery, and execution
//! at runtime. Built-in tools in [`builtin`] provide calculator, web search,
//! file reading, and shell execution capabilities.

pub mod builtin;
pub mod registry;
pub mod types;

pub use registry::ToolRegistry;
pub use types::*;
