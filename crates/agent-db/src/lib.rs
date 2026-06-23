//! # agent-db
//!
//! Database layer for the AI Agent application, built on `sqlx` with SQLite.
//!
//! This crate provides:
//!
//! - **Models** -- Row structs (`ConfigRow`, `SessionRow`, `MessageRow`, etc.)
//!   that map to SQLite table rows with `sqlx::FromRow` derive.
//! - **Repository layer** -- Type-safe CRUD operations for each entity
//!   (config, sessions, messages, channels, workflows, tasks).
//! - **Connection pool** -- SQLite connection pool with WAL mode, foreign
//!   keys, and performance pragmas enabled automatically.
//! - **Migrations** -- Embedded SQL migrations run automatically on startup.
//!
//! ## Initialization
//!
//! Call [`init_db`] once at application startup to create the connection pool
//! and run all pending migrations:
//!
//! ```no_run
//! # use agent_db::init_db;
//! # async fn example() -> Result<(), agent_db::error::DbError> {
//! let pool = init_db("data/agent.db").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Repository Pattern
//!
//! Each entity has a dedicated repository struct (e.g. [`repo::ConfigRepo`],
//! [`repo::SessionRepo`]) that wraps the connection pool and provides typed
//! CRUD methods. All repository operations are async and return
//! `Result<T, DbError>`.
//!
//! ## Models
//!
//! Row structs in [`models`] use `sqlx::FromRow` for automatic mapping from
//! query results. They also derive `Serialize`/`Deserialize` for JSON
//! serialization in API responses.
//!
//! ## Database Pragmas
//!
//! On initialization, the following SQLite pragmas are set:
//! - `journal_mode=WAL` -- better concurrent read performance
//! - `foreign_keys=ON` -- enforce referential integrity
//! - `cache_size=-8000` -- 8 MB page cache
//! - `synchronous=NORMAL` -- balance between safety and performance
#![deny(clippy::perf)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::cast_possible_truncation)]

pub mod error;
pub mod models;
pub mod pool;
pub mod repo;

use sqlx::migrate::Migrator;
use sqlx::SqlitePool;
use std::path::Path;
use tracing::info;

static MIGRATOR: Migrator = sqlx::migrate!("./src/migrations");

/// Run database migrations
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), error::DbError> {
    info!("Running database migrations...");
    MIGRATOR.run(pool).await?;
    info!("Database migrations completed.");
    Ok(())
}

/// Initialize the database: create pool, run migrations
pub async fn init_db(db_path: &str) -> Result<SqlitePool, error::DbError> {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(db_path).parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                error::DbError::Connection(format!(
                    "Failed to create database directory: {e}"
                ))
            })?;
        }
    }

    let database_url = format!("sqlite:{db_path}?mode=rwc");
    let pool = pool::create_pool(&database_url).await?;

    // Enable WAL mode for better concurrent performance
    sqlx::query("PRAGMA journal_mode=WAL;")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys=ON;")
        .execute(&pool)
        .await?;

    // Performance tuning
    sqlx::query("PRAGMA cache_size=-8000;")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA synchronous=NORMAL;")
        .execute(&pool)
        .await?;

    run_migrations(&pool).await?;

    Ok(pool)
}
