use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub name: String,
    pub cron_expression: String,
    pub prompt: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_model() -> String {
    "deepseek-chat".into()
}
fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub name: Option<String>,
    pub cron_expression: Option<String>,
    pub prompt: Option<String>,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub enabled: Option<bool>,
}

/// `GET /api/tasks` -- list all scheduled tasks ordered by most recently updated.
pub async fn list(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let tasks = state.task_repo.list().await?;
    Ok(Json(json!(tasks)))
}

/// `POST /api/tasks` -- create a new scheduled task. Requires `name`,
/// `cron_expression` (5-field format), and `prompt`. Defaults:
/// model=`"deepseek-chat"`, enabled=`true`.
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateTaskRequest>,
) -> Result<Json<Value>, ApiError> {
    let id = Uuid::new_v4().to_string();

    let task = agent_db::models::ScheduledTaskRow {
        id,
        name: body.name,
        cron_expression: body.cron_expression,
        prompt: body.prompt,
        session_id: body.session_id,
        model: body.model,
        enabled: body.enabled,
        created_at: String::new(),
        updated_at: String::new(),
    };

    state.task_repo.create(&task).await?;
    Ok(Json(json!(task)))
}

/// `GET /api/tasks/{id}` -- return a single task by ID. Returns 404 if not found.
pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let task = state
        .task_repo
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Task '{}' not found", id)))?;
    Ok(Json(json!(task)))
}

/// `PUT /api/tasks/{id}` -- update task fields. All fields optional.
/// Changing the cron expression will take effect on the next scheduler tick.
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateTaskRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut task = state
        .task_repo
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Task '{}' not found", id)))?;

    if let Some(name) = body.name { task.name = name; }
    if let Some(ce) = body.cron_expression { task.cron_expression = ce; }
    if let Some(prompt) = body.prompt { task.prompt = prompt; }
    if let Some(sid) = body.session_id { task.session_id = Some(sid); }
    if let Some(model) = body.model { task.model = model; }
    if let Some(enabled) = body.enabled { task.enabled = enabled; }

    state.task_repo.update(&task).await?;
    Ok(Json(json!(task)))
}

/// `DELETE /api/tasks/{id}` -- delete a task and its execution logs (cascade).
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state.task_repo.delete(&id).await?;
    Ok(Json(json!({ "deleted": true })))
}

/// `POST /api/tasks/{id}/run` -- trigger a task execution immediately,
/// bypassing the cron schedule. Returns the LLM response.
pub async fn run_now(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let task = state
        .task_repo
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Task '{}' not found", id)))?;

    let result = state
        .scheduler
        .execute_task(&task.id, &task.prompt, &task.model)?;

    // Broadcast task_executed event
    state.broadcast_event("task_executed", json!({
        "task_id": id,
        "name": task.name,
        "result_preview": &result[..result.len().min(200)],
    }));

    // Email notification
    if let Some(ref notifier) = state.email_notifier {
        let notify = notifier.clone();
        let name = task.name.clone();
        let output = result.clone();
        tokio::spawn(async move {
            notify.notify_task_complete(&name, "success", &output).await;
        });
    }

    Ok(Json(json!({ "task_id": id, "result": result })))
}

/// `GET /api/tasks/{id}/logs` -- return the last 50 execution log entries
/// for a task, ordered by most recent first.
pub async fn logs(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let logs = state.task_repo.list_logs(&id, 50).await?;
    Ok(Json(json!(logs)))
}
