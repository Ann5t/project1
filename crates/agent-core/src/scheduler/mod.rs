//! Cron-based task scheduler.
//!
//! The [`TaskSchedulerEngine`] loads enabled scheduled tasks from the database
//! and runs them on their cron schedules. Each task sends a prompt to the LLM
//! and records the result in the `task_logs` table. Manual trigger (via
//! [`execute_task`](TaskSchedulerEngine::execute_task)) is also supported.

pub mod engine;

pub use engine::TaskSchedulerEngine;
