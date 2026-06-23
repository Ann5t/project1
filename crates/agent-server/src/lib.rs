#![recursion_limit = "512"]
// agent-server: server crate lint configuration
// We allow noisy pedantic lints that are not critical for a server binary
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::unnecessary_literal_bound)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::similar_names)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::disallowed_methods)]
#![allow(clippy::borrow_deref_ref)]
#![allow(clippy::needless_borrows_for_generic_args)]

//! # agent-server
//!
//! HTTP server crate for the AI Agent application, built on `axum` and `tokio`.
//!
//! This crate wires together:
//!
//! - **Routes** -- REST API handlers for health, config, sessions, chat, channels,
//!   workflows, tasks, monitoring, and publishing.
//! - **Channels** -- Platform integrations (Feishu/Lark, QQ Bot, WeChat Work)
//!   implementing the `Channel` trait.
//! - **State** -- Shared `AppState` injected into all handlers via Axum's
//!   state extractor, containing database pools, LLM client, tool registry,
//!   session manager, workflow engine, scheduler, and monitoring counters.
//! - **Middleware** -- Request counting and error recording for the monitoring
//!   dashboard.
//! - **Config** -- Environment-variable-based `ServerConfig` for bind address,
//!   database path, and logging.
//!
//! The binary entry point is `main.rs`, which calls `agent_db::init_db` and
//! then builds the Axum router.

pub mod channel;
pub mod config;
pub mod error;
pub mod middleware;
pub mod notifications;
pub mod routes;
pub mod state;
