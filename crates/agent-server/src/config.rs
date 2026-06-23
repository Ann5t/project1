//! Server configuration loaded from environment variables.
//!
//! All fields have sensible defaults so the server can start with no
//! environment variables set.

use std::env;

/// Server configuration built from environment variables at startup.
///
/// See the project README for a complete environment variable reference.
pub struct ServerConfig {
    pub bind_address: String,
    pub database_path: String,
    pub frontend_dir: String,
    pub rust_log: String,
}

impl ServerConfig {
    /// Build a `ServerConfig` from environment variables with sensible defaults.
    pub fn from_env() -> Self {
        Self {
            bind_address: env::var("BIND_ADDRESS")
                .unwrap_or_else(|_| "0.0.0.0:3000".into()),
            database_path: env::var("DATABASE_PATH")
                .unwrap_or_else(|_| "data/agent.db".into()),
            frontend_dir: env::var("FRONTEND_DIR")
                .unwrap_or_else(|_| "crates/agent-frontend".into()),
            rust_log: env::var("RUST_LOG")
                .unwrap_or_else(|_| "info".into()),
        }
    }
}
