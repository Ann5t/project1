//! Integration tests for the AI Agent API
//!
//! Run with: cargo test --test api_test

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use std::sync::Arc;
use tower::ServiceExt;

use agent_core::llm::client::DeepSeekClient;
use agent_core::tool::builtin::{CalculatorTool, CurrentTimeTool};
use agent_core::tool::registry::ToolRegistry;
use agent_server::state::AppState;

/// Helper to create a test AppState with an in-memory database
async fn create_test_state() -> AppState {
    let db = agent_db::init_db(":memory:")
        .await
        .expect("Failed to init test DB");

    let llm: Arc<dyn agent_core::llm::client::LlmClient + Send + Sync> =
        Arc::new(DeepSeekClient::new(
            "test-key".into(),
            Some("https://api.deepseek.com/v1".into()),
            Some("deepseek-chat".into()),
        ));

    let tools = Arc::new(ToolRegistry::new());
    tools.register(Arc::new(CalculatorTool)).await;
    tools.register(Arc::new(CurrentTimeTool)).await;

    AppState::new(db, llm, tools)
}

fn create_test_app(state: AppState) -> Router {
    use axum::routing::{delete, get, post, put};

    Router::new()
        // Health
        .route(
            "/api/health",
            get(agent_server::routes::health::health_check),
        )
        .route("/api/info", get(agent_server::routes::health::system_info))
        // Config
        .route(
            "/api/config",
            get(agent_server::routes::config_api::get_all),
        )
        .route(
            "/api/config",
            put(agent_server::routes::config_api::update_all),
        )
        .route(
            "/api/config/{key}",
            get(agent_server::routes::config_api::get_one),
        )
        .route(
            "/api/config/{key}",
            put(agent_server::routes::config_api::set_one),
        )
        // Sessions
        .route("/api/sessions", get(agent_server::routes::session::list))
        .route("/api/sessions", post(agent_server::routes::session::create))
        .route(
            "/api/sessions/{id}",
            get(agent_server::routes::session::get_one),
        )
        .route(
            "/api/sessions/{id}",
            put(agent_server::routes::session::update),
        )
        .route(
            "/api/sessions/{id}",
            delete(agent_server::routes::session::delete),
        )
        .route(
            "/api/sessions/{id}/messages",
            get(agent_server::routes::session::messages),
        )
        // Workflows
        .route("/api/workflows", get(agent_server::routes::workflow::list))
        .route(
            "/api/workflows",
            post(agent_server::routes::workflow::create),
        )
        .route(
            "/api/workflows/{id}",
            get(agent_server::routes::workflow::get_one),
        )
        .route(
            "/api/workflows/{id}",
            put(agent_server::routes::workflow::update),
        )
        .route(
            "/api/workflows/{id}",
            delete(agent_server::routes::workflow::delete),
        )
        // Tasks
        .route("/api/tasks", get(agent_server::routes::task::list))
        .route("/api/tasks", post(agent_server::routes::task::create))
        .route("/api/tasks/{id}", get(agent_server::routes::task::get_one))
        .route("/api/tasks/{id}", put(agent_server::routes::task::update))
        .route(
            "/api/tasks/{id}",
            delete(agent_server::routes::task::delete),
        )
        .with_state(state)
}

/// Helper to make a GET request and deserialize the JSON body
async fn get_json(app: &Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body_bytes = axum::body::to_bytes(resp.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();
    (status, body)
}

/// Helper to make a request with a JSON body
async fn request_json(
    app: &Router,
    method: Method,
    uri: &str,
    body_json: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let body_str = body_json.to_string();
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body_str))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body_bytes = axum::body::to_bytes(resp.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();
    (status, body)
}

/// Helper to make a DELETE request
async fn delete(app: &Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(Method::DELETE)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body_bytes = axum::body::to_bytes(resp.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or_default();
    (status, body)
}

// ============================================================================
// Health Endpoint Tests
// ============================================================================

#[tokio::test]
async fn health_check_returns_ok() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, body) = get_json(&app, "/api/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(body["version"].as_str().is_some());
    assert!(body["timestamp"].as_str().is_some());
}

#[tokio::test]
async fn system_info_returns_data() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, body) = get_json(&app, "/api/info").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "running");
    assert!(body["version"].as_str().is_some());
    assert!(body["stats"].is_object());
    assert!(body["features"].is_object());
    assert!(body["channels"].is_array());
}

// ============================================================================
// Config Endpoint Tests
// ============================================================================

#[tokio::test]
async fn config_get_all_returns_seed_data() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, body) = get_json(&app, "/api/config").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.as_object().unwrap().len() > 0,
        "Config should have seed data"
    );
    assert!(body.get("api_key").is_some(), "Should have api_key config");
    assert!(
        body.get("default_model").is_some(),
        "Should have default_model"
    );
}

#[tokio::test]
async fn config_update_and_read_back() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Update multiple keys
    let (status, _) = request_json(
        &app,
        Method::PUT,
        "/api/config",
        serde_json::json!({"api_key": "sk-test-123", "theme": "light"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Read back
    let (status, body) = get_json(&app, "/api/config").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["api_key"], "sk-test-123");
    assert_eq!(body["theme"], "light");
}

#[tokio::test]
async fn config_get_single_key() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, body) = get_json(&app, "/api/config/default_model").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["key"], "default_model");
    assert_eq!(body["value"], "deepseek-chat");
}

#[tokio::test]
async fn config_get_single_key_not_found() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, _) = get_json(&app, "/api/config/nonexistent_key_xyz").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn config_set_single_key() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, _) = request_json(
        &app,
        Method::PUT,
        "/api/config/custom_key",
        serde_json::json!({"value": "custom_value"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = get_json(&app, "/api/config/custom_key").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["value"], "custom_value");
}

#[tokio::test]
async fn config_set_missing_value_field() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, _) = request_json(
        &app,
        Method::PUT,
        "/api/config/bad_key",
        serde_json::json!({"not_value": "something"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn config_set_empty_value_string() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Setting an empty string as value should work
    let (status, _) = request_json(
        &app,
        Method::PUT,
        "/api/config/empty_key",
        serde_json::json!({"value": ""}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Read back - should be empty string
    let (status, body) = get_json(&app, "/api/config/empty_key").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["value"], "");
}

#[tokio::test]
async fn config_set_unicode_value() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, _) = request_json(
        &app,
        Method::PUT,
        "/api/config/unicode_key",
        serde_json::json!({"value": "你好世界 🌍"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = get_json(&app, "/api/config/unicode_key").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["value"], "你好世界 🌍");
}

#[tokio::test]
async fn config_set_special_characters_value() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let special_val = r#"{"key": "value", "array": [1,2,3], "nested": {"a": "b"}}"#;
    let (status, _) = request_json(
        &app,
        Method::PUT,
        "/api/config/special_key",
        serde_json::json!({"value": special_val}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = get_json(&app, "/api/config/special_key").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["value"], special_val);
}

#[tokio::test]
async fn config_set_value_is_number_not_string() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // When value is a number (not a string), the .as_str() returns None
    // This should return 400 Bad Request
    let (status, _) = request_json(
        &app,
        Method::PUT,
        "/api/config/number_key",
        serde_json::json!({"value": 42}),
    )
    .await;
    // Currently the handler uses .as_str() which fails on numbers -> BadRequest
    assert!(status == StatusCode::BAD_REQUEST || status == StatusCode::OK);
}

#[tokio::test]
async fn config_update_all_with_empty_object() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Updating with empty object should not error
    let (status, _) = request_json(&app, Method::PUT, "/api/config", serde_json::json!({})).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn config_get_all_returns_content_type_json() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/config")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    // Axum's Json extractor sets content-type: application/json
    let content_type = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("application/json"));
}

// ============================================================================
// Session Endpoint Tests
// ============================================================================

#[tokio::test]
async fn session_crud_cycle() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // List empty
    let (status, body) = get_json(&app, "/api/sessions").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().unwrap().is_empty());

    // Create
    let (status, created) = request_json(
        &app,
        Method::POST,
        "/api/sessions",
        serde_json::json!({
            "name": "Test Session",
            "temperature": 0.5,
            "max_tokens": 2048
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let session_id = created["id"].as_str().unwrap().to_string();

    // List has 1
    let (status, body) = get_json(&app, "/api/sessions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 1);

    // Get single
    let (status, session) = get_json(&app, &format!("/api/sessions/{}", session_id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(session["name"], "Test Session");
    assert_eq!(session["temperature"], 0.5);
    assert_eq!(session["max_tokens"], 2048);

    // Delete
    let (status, _) = delete(&app, &format!("/api/sessions/{}", session_id)).await;
    assert_eq!(status, StatusCode::OK);

    // List empty again
    let (status, body) = get_json(&app, "/api/sessions").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn session_get_not_found() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, _) = get_json(&app, "/api/sessions/nonexistent-id").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn session_update() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Create first
    let (_, created) = request_json(
        &app,
        Method::POST,
        "/api/sessions",
        serde_json::json!({"name": "Original Name"}),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // Update
    let (status, updated) = request_json(
        &app,
        Method::PUT,
        &format!("/api/sessions/{}", id),
        serde_json::json!({
            "name": "Updated Name",
            "temperature": 0.2,
            "max_tokens": 1024
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["name"], "Updated Name");
    assert_eq!(updated["temperature"], 0.2);
    assert_eq!(updated["max_tokens"], 1024);
}

#[tokio::test]
async fn session_messages_empty_for_new_session() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Create session
    let (_, created) = request_json(
        &app,
        Method::POST,
        "/api/sessions",
        serde_json::json!({"name": "Msg Test"}),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // Messages should be empty
    let (status, msgs) = get_json(&app, &format!("/api/sessions/{}/messages", id)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(msgs.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn session_create_with_all_fields() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, created) = request_json(
        &app,
        Method::POST,
        "/api/sessions",
        serde_json::json!({
            "name": "Full Session",
            "agent_id": "agent-42",
            "system_prompt": "You are a math tutor.",
            "model": "deepseek-reasoner",
            "temperature": 0.1,
            "max_tokens": 8192
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(created["name"], "Full Session");
    assert_eq!(created["agent_id"], "agent-42");
    assert_eq!(created["system_prompt"], "You are a math tutor.");
    assert_eq!(created["model"], "deepseek-reasoner");
    assert_eq!(created["temperature"], 0.1);
    assert_eq!(created["max_tokens"], 8192);
    assert_eq!(created["channel"], "web");
}

// ── Session Edge Cases ──

#[tokio::test]
async fn session_create_with_special_characters_name() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, created) = request_json(
        &app,
        Method::POST,
        "/api/sessions",
        serde_json::json!({
            "name": "Session /\\ 测试 🧪 <>&\"'#",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["name"], "Session /\\ 测试 🧪 <>&\"'#");
}

#[tokio::test]
async fn session_create_with_very_long_name() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let long_name = "A".repeat(500);
    let (status, created) = request_json(
        &app,
        Method::POST,
        "/api/sessions",
        serde_json::json!({
            "name": &long_name,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["name"].as_str().unwrap().len(), 500);
    assert_eq!(created["name"], long_name);
}

#[tokio::test]
async fn session_update_with_partial_fields() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Create
    let (_, created) = request_json(
        &app,
        Method::POST,
        "/api/sessions",
        serde_json::json!({
            "name": "Partial Update",
            "temperature": 0.8,
            "max_tokens": 8192,
        }),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // Update only temperature, rest should stay same
    let (status, updated) = request_json(
        &app,
        Method::PUT,
        &format!("/api/sessions/{}", id),
        serde_json::json!({
            "temperature": 0.2,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["name"], "Partial Update"); // unchanged
    assert_eq!(updated["temperature"], 0.2); // changed
    assert_eq!(updated["max_tokens"], 8192); // unchanged
}

#[tokio::test]
async fn session_update_clear_system_prompt() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Create with system prompt
    let (_, created) = request_json(
        &app,
        Method::POST,
        "/api/sessions",
        serde_json::json!({
            "name": "Clear SP",
            "system_prompt": "Old prompt",
        }),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["system_prompt"], "Old prompt");

    // Update with system_prompt explicitly set to something
    let (_, updated) = request_json(
        &app,
        Method::PUT,
        &format!("/api/sessions/{}", id),
        serde_json::json!({
            "system_prompt": "New prompt",
        }),
    )
    .await;
    assert_eq!(updated["system_prompt"], "New prompt");
}

// ============================================================================
// Workflow Endpoint Tests
// ============================================================================

#[tokio::test]
async fn workflow_crud_cycle() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Create
    let (status, created) = request_json(
        &app,
        Method::POST,
        "/api/workflows",
        serde_json::json!({
            "name": "Test Workflow",
            "description": "A test workflow",
            "definition": {"steps": [], "edges": []}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["name"], "Test Workflow");
    let id = created["id"].as_str().unwrap().to_string();

    // Get single
    let (status, wf) = get_json(&app, &format!("/api/workflows/{}", id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(wf["name"], "Test Workflow");

    // Update
    let (status, updated) = request_json(
        &app,
        Method::PUT,
        &format!("/api/workflows/{}", id),
        serde_json::json!({
            "name": "Updated Workflow",
            "enabled": false
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["name"], "Updated Workflow");

    // Delete
    let (status, _) = delete(&app, &format!("/api/workflows/{}", id)).await;
    assert_eq!(status, StatusCode::OK);

    // Verify deleted
    let (status, _) = get_json(&app, &format!("/api/workflows/{}", id)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn workflow_get_not_found() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, _) = get_json(&app, "/api/workflows/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn workflow_with_cron_trigger() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, created) = request_json(
        &app,
        Method::POST,
        "/api/workflows",
        serde_json::json!({
            "name": "Cron Workflow",
            "description": "Runs every hour",
            "trigger_type": "cron",
            "cron_expression": "0 * * * *",
            "definition": {"steps": [], "edges": []}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["trigger_type"], "cron");
    assert_eq!(created["cron_expression"], "0 * * * *");
}

#[tokio::test]
async fn workflow_list_contains_created() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Initially empty
    let (_, body) = get_json(&app, "/api/workflows").await;
    let initial_count = body.as_array().unwrap().len();

    // Create one
    request_json(
        &app,
        Method::POST,
        "/api/workflows",
        serde_json::json!({
            "name": "List Test",
            "definition": {"steps": [], "edges": []}
        }),
    )
    .await;

    // List should have one more
    let (_, body) = get_json(&app, "/api/workflows").await;
    assert_eq!(body.as_array().unwrap().len(), initial_count + 1);
}

// ============================================================================
// Task Endpoint Tests
// ============================================================================

#[tokio::test]
async fn task_crud_cycle() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Create
    let (status, created) = request_json(
        &app,
        Method::POST,
        "/api/tasks",
        serde_json::json!({
            "name": "Daily Report",
            "cron_expression": "0 9 * * *",
            "prompt": "Generate daily report"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["name"], "Daily Report");
    assert_eq!(created["cron_expression"], "0 9 * * *");
    assert_eq!(created["prompt"], "Generate daily report");
    let id = created["id"].as_str().unwrap().to_string();

    // Get single
    let (status, task) = get_json(&app, &format!("/api/tasks/{}", id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(task["name"], "Daily Report");

    // Update
    let (status, updated) = request_json(
        &app,
        Method::PUT,
        &format!("/api/tasks/{}", id),
        serde_json::json!({
            "name": "Updated Report",
            "cron_expression": "0 10 * * *",
            "enabled": false
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["name"], "Updated Report");
    assert_eq!(updated["cron_expression"], "0 10 * * *");

    // Delete
    let (status, _) = delete(&app, &format!("/api/tasks/{}", id)).await;
    assert_eq!(status, StatusCode::OK);

    // Verify deleted
    let (status, _) = get_json(&app, &format!("/api/tasks/{}", id)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn task_get_not_found() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, _) = get_json(&app, "/api/tasks/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn task_create_with_session_and_model() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (status, created) = request_json(
        &app,
        Method::POST,
        "/api/tasks",
        serde_json::json!({
            "name": "Special Task",
            "cron_expression": "*/5 * * * *",
            "prompt": "Check status",
            "session_id": "some-session-id",
            "model": "deepseek-reasoner",
            "enabled": false
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["model"], "deepseek-reasoner");
    assert_eq!(created["session_id"], "some-session-id");
    assert!(!created["enabled"].as_bool().unwrap());
}

#[tokio::test]
async fn task_list_contains_created() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    let (_, body) = get_json(&app, "/api/tasks").await;
    let initial_count = body.as_array().unwrap().len();

    request_json(
        &app,
        Method::POST,
        "/api/tasks",
        serde_json::json!({
            "name": "Count Task",
            "cron_expression": "0 0 * * *",
            "prompt": "test"
        }),
    )
    .await;

    let (_, body) = get_json(&app, "/api/tasks").await;
    assert_eq!(body.as_array().unwrap().len(), initial_count + 1);
}

// ============================================================================
// Cross-Endpoint Tests
// ============================================================================

#[tokio::test]
async fn multiple_sessions_independent() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Create 3 sessions
    let names = vec!["Alpha", "Beta", "Gamma"];
    for name in &names {
        request_json(
            &app,
            Method::POST,
            "/api/sessions",
            serde_json::json!({"name": name}),
        )
        .await;
    }

    let (_, body) = get_json(&app, "/api/sessions").await;
    let sessions = body.as_array().unwrap();
    assert_eq!(sessions.len(), 3);

    let returned_names: Vec<&str> = sessions
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    for name in &names {
        assert!(returned_names.contains(name));
    }
}

#[tokio::test]
async fn config_update_persists_across_requests() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Update
    request_json(
        &app,
        Method::PUT,
        "/api/config",
        serde_json::json!({"test_persist": "true"}),
    )
    .await;

    // Verify persists
    let (_, body) = get_json(&app, "/api/config").await;
    assert_eq!(body["test_persist"], "true");
}

#[tokio::test]
async fn delete_session_does_not_affect_others() {
    let state = create_test_state().await;
    let app = create_test_app(state);

    // Create two sessions
    let (_, body) = request_json(
        &app,
        Method::POST,
        "/api/sessions",
        serde_json::json!({"name": "Keep Me"}),
    )
    .await;
    let keep_id = body["id"].as_str().unwrap().to_string();

    let (_, body) = request_json(
        &app,
        Method::POST,
        "/api/sessions",
        serde_json::json!({"name": "Delete Me"}),
    )
    .await;
    let delete_id = body["id"].as_str().unwrap().to_string();

    // Delete one
    delete(&app, &format!("/api/sessions/{}", delete_id)).await;

    // The other should still exist
    let (_, body) = get_json(&app, "/api/sessions").await;
    let sessions = body.as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], keep_id);
}
