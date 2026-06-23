//! Repository layer -- type-safe CRUD operations for each database entity.
//!
//! Each repository wraps a [`SqlitePool`](sqlx::SqlitePool) and provides
//! async methods for creating, reading, updating, and deleting the
//! corresponding entity. All operations return `Result<T, DbError>`.
//!
//! ## Available Repositories
//!
//! | Repository | Entity | Description |
//! |------------|--------|-------------|
//! | [`ConfigRepo`] | `config` | Key-value configuration store |
//! | [`SessionRepo`] | `sessions` | Conversation sessions |
//! | [`MessageRepo`] | `messages` | Chat messages within a session |
//! | [`ChannelRepo`] | `channels` | Platform channel configurations |
//! | [`WorkflowRepo`] | `workflows` | Workflow definitions and run history |
//! | [`TaskRepo`] | `scheduled_tasks` | Cron-scheduled tasks and execution logs |

pub mod channel_repo;
pub mod config_repo;
pub mod message_repo;
pub mod session_repo;
pub mod task_repo;
pub mod workflow_repo;

pub use channel_repo::ChannelRepo;
pub use config_repo::ConfigRepo;
pub use message_repo::MessageRepo;
pub use session_repo::SessionRepo;
pub use task_repo::TaskRepo;
pub use workflow_repo::WorkflowRepo;
