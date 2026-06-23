//! LLM (Large Language Model) integration layer.
//!
//! Provides an abstract [`LlmClient`] trait that any model provider can implement,
//! along with a concrete [`DeepSeekClient`] supporting both standard and streaming
//! chat completions via the OpenAI-compatible API.
//!
//! The [`stats`] module tracks global LLM API call counters for the monitoring
//! dashboard.

pub mod client;
pub mod stats;
pub mod types;

pub use client::{DeepSeekClient, LlmClient};
pub use types::*;
