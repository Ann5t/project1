//! Email/SMTP notification support.
//!
//! The [`email::EmailNotifier`] sends email notifications for workflow
//! completions, task executions, and system errors via SMTP. It is configured
//! through database config keys (see [`email::EmailNotifier::from_config`]).
//!
//! A global singleton ([`email::GLOBAL_EMAIL_NOTIFIER`]) is initialized at
//! startup in `main.rs` and used by error recording, workflow, and task
//! handlers to send notifications without threading an `AppState` through
//! every call site.

pub mod email;
