//! Route handler modules for the REST API.
//!
//! Each module corresponds to a resource group in the API:
//!
//! | Module | Prefix | Description |
//! |--------|--------|-------------|
//! | [`health`] | `/api/health`, `/api/info` | Health check and system info |
//! | [`config_api`] | `/api/config` | Key-value configuration CRUD |
//! | [`session`] | `/api/sessions` | Conversation session CRUD |
//! | [`chat`] | `/api/chat` | Message send (non-streaming and SSE) |
//! | [`channel`] | `/api/channels` | Channel management + Feishu callback |
//! | [`workflow`] | `/api/workflows` | Workflow definition CRUD + execution |
//! | [`task`] | `/api/tasks` | Scheduled task CRUD + logs |
//! | [`monitor`] | `/api/monitor`, `/monitor` | Monitoring stats + dashboard |
//! | [`search`] | `/api/search` | Search sessions and messages |
//! | [`export`] | `/api/export` | Session and workflow data export |
//! | [`publish`] | `/p/{id}` | Published result pages |
//! | [`ws`] | `/api/ws` | WebSocket real-time events |

pub mod auth;
pub mod backup;
pub mod channel;
pub mod chat;
pub mod config_api;
pub mod export;
pub mod health;
pub mod metrics;
pub mod monitor;
pub mod notifications;
pub mod openapi;
pub mod publish;
pub mod search;
pub mod session;
pub mod task;
pub mod workflow;
pub mod ws;
