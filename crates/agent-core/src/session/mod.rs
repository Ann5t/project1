//! Session management module.
//!
//! The [`SessionManager`] is the heart of the agent loop. It loads conversation
//! history from the database, constructs LLM requests with tool definitions,
//! handles tool-call execution via the `ToolRegistry`, and saves assistant
//! responses. Both non-streaming and streaming (SSE) APIs are supported.

pub mod manager;

pub use manager::SessionManager;
