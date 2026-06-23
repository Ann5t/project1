use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateWorkflowRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub definition: Value,
    #[serde(default = "default_trigger")]
    pub trigger_type: String,
    pub cron_expression: Option<String>,
}

fn default_trigger() -> String {
    "manual".into()
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkflowRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub definition: Option<Value>,
    pub trigger_type: Option<String>,
    pub cron_expression: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct RunsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    20
}

/// `GET /api/workflows` -- list all workflow definitions ordered by most
/// recently updated.
pub async fn list(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let workflows = state.workflow_repo.list().await?;
    Ok(Json(json!(workflows)))
}

/// `POST /api/workflows` -- create a new workflow. The `definition` field
/// contains the DAG (steps + edges in JSON). `trigger_type` can be `"manual"`
/// or `"cron"` (requires `cron_expression`).
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateWorkflowRequest>,
) -> Result<Json<Value>, ApiError> {
    let id = Uuid::new_v4().to_string();

    let wf = agent_db::models::WorkflowRow {
        id,
        name: body.name,
        description: body.description,
        definition: body.definition.to_string(),
        trigger_type: body.trigger_type,
        cron_expression: body.cron_expression,
        enabled: true,
        last_run_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    };

    state.workflow_repo.create(&wf).await?;
    Ok(Json(json!(wf)))
}

/// `GET /api/workflows/{id}` -- return a single workflow by ID. Returns
/// 404 if not found.
pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let wf = state
        .workflow_repo
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Workflow '{}' not found", id)))?;
    Ok(Json(json!(wf)))
}

/// `PUT /api/workflows/{id}` -- update workflow fields. All fields optional.
/// The `definition` field, if provided, replaces the entire DAG.
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateWorkflowRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut wf = state
        .workflow_repo
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Workflow '{}' not found", id)))?;

    if let Some(name) = body.name {
        wf.name = name;
    }
    if let Some(desc) = body.description {
        wf.description = desc;
    }
    if let Some(def) = body.definition {
        wf.definition = def.to_string();
    }
    if let Some(tt) = body.trigger_type {
        wf.trigger_type = tt;
    }
    if let Some(ce) = body.cron_expression {
        wf.cron_expression = Some(ce);
    }
    if let Some(en) = body.enabled {
        wf.enabled = en;
    }

    state.workflow_repo.update(&wf).await?;
    Ok(Json(json!(wf)))
}

/// `DELETE /api/workflows/{id}` -- delete a workflow and its execution
/// records (cascade).
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state.workflow_repo.delete(&id).await?;
    Ok(Json(json!({ "deleted": true })))
}

/// `POST /api/workflows/{id}/run` -- execute a workflow immediately. The
/// definition is parsed as a DAG, topologically sorted, and executed in
/// parallel batches. Results include per-step status and output.
/// `POST /api/workflows/{id}/run` — execute a workflow definition.
///
/// The 3-phase async body is extracted into [`run_all`] and wrapped in an
/// [`AssertSendFuture`] newtype that unsafely asserts `Send`.  This is
/// necessary because Rust's opaque-`impl Future` auto-trait inference hits
/// an HRTB limitation ("Send is not general enough") when the future
/// captures self-ownership references (e.g. `&self.pool`) across multiple
/// `.await` points.  The future IS `Send` — all captured data is owned and
/// `Send + Sync` — the compiler simply cannot prove it through the opaque
/// type.
pub fn run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AssertSendFuture<impl std::future::Future<Output = Result<Json<Value>, ApiError>>> {
    AssertSendFuture::new(run_all(state, id))
}

// ── newtype that unsafely asserts Send ─────────────────────────────────

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A [`Future`] wrapper that unsafely asserts `Send` for its inner future.
///
/// # Safety
/// The caller MUST ensure the inner future is actually `Send`.  For
/// [`run_all`] this holds because every captured value is `Send + Sync`
/// (the pool, repo handles, strings, etc.).
pub struct AssertSendFuture<F> {
    inner: F,
}

impl<F> AssertSendFuture<F> {
    pub fn new(inner: F) -> Self {
        Self { inner }
    }
}

// SAFETY: the caller (run) guarantees that F is Send.
unsafe impl<F> Send for AssertSendFuture<F> {}

impl<F: Future> Future for AssertSendFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: pin projection — we only access inner through the pinned
        // wrapper, which is sound because AssertSendFuture is not Unpin
        // unless F is.
        let inner = unsafe { self.map_unchecked_mut(|s| &mut s.inner) };
        inner.poll(cx)
    }
}

// ── the actual async body ──────────────────────────────────────────────

async fn run_all(state: AppState, id: String) -> Result<Json<Value>, ApiError> {
    let wf = state
        .workflow_repo
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Workflow '{}' not found", id)))?;

    let definition: agent_core::workflow::types::WorkflowDefinition =
        serde_json::from_str(&wf.definition)
            .map_err(|e| ApiError::BadRequest(format!("Invalid workflow definition: {}", e)))?;

    state.broadcast_event(
        "workflow_started",
        json!({
            "workflow_id": id,
            "name": wf.name,
        }),
    );

    let result = state.workflow_engine.execute(&id, &definition).await?;

    state.workflow_repo.touch_run(&id).await?;

    let status_str = match result.status {
        agent_core::workflow::types::StepStatus::Success => "success",
        agent_core::workflow::types::StepStatus::Error => "error",
        agent_core::workflow::types::StepStatus::Skipped => "skipped",
        agent_core::workflow::types::StepStatus::Pending => "pending",
        agent_core::workflow::types::StepStatus::Running => "running",
    };
    state.broadcast_event(
        "workflow_completed",
        json!({
            "workflow_id": id,
            "name": wf.name,
            "status": status_str,
        }),
    );

    if let Some(ref notifier) = state.email_notifier {
        let result_url = format!("http://localhost:3000/workflows/{}", id);
        let notify = notifier.clone();
        let name = wf.name.clone();
        let status = status_str.to_string();
        tokio::spawn(async move {
            notify
                .notify_workflow_complete(&name, &status, &result_url)
                .await;
        });
    }

    Ok(Json(json!(result)))
}

/// `GET /api/workflows/{id}/runs` -- return execution history for a
/// workflow. Accepts `?limit=N` (default 20).
pub async fn runs(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<RunsQuery>,
) -> Result<Json<Value>, ApiError> {
    let runs = state.workflow_repo.list_runs(&id, query.limit).await?;
    Ok(Json(json!(runs)))
}
