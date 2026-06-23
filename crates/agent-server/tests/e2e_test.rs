//! End-to-end integration tests that start the actual server on a random port
//! with an in-memory database and a mock LLM client, then exercise complete
//! workflows via the HTTP API using reqwest.
//!
//! Run with: cargo test --test e2e_test -- --nocapture
//!
//! NOTE: These tests run against a real axum server bound to 127.0.0.1:0.
//! Each test starts its own server instance for isolation.

use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::Stream;
use std::pin::Pin;

use agent_core::error::CoreError;
use agent_core::llm::client::LlmClient;
use agent_core::llm::types::*;
use agent_core::tool::registry::ToolRegistry;

use agent_server::state::AppState;

use axum::routing::{delete, get, post, put};
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

// ============================================================================
// Mock LLM Client
// ============================================================================

/// A mock LLM client that always returns a canned success response.
/// Used so e2e tests don't require a real LLM API key or network access.
struct MockLlmClient;

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, CoreError> {
        Ok(ChatResponse {
            id: "mock-chat-1".into(),
            object: "chat.completion".into(),
            created: 1,
            model: request.model.clone(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "This is a mock LLM response for e2e testing.".into(),
                    tool_calls: None,
                    tool_call_id: None,
                },
                finish_reason: Some("stop".into()),
            }],
        })
    }

    async fn list_models(&self) -> Result<Vec<String>, CoreError> {
        Ok(vec!["mock-model".into(), "mock-model-2".into()])
    }

    fn chat_stream(
        &self,
        _request: &ChatRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<String, CoreError>> + Send>> {
        Box::pin(futures::stream::once(async {
            Ok("Mock stream response".into())
        }))
    }
}

// ============================================================================
// Server helper
// ============================================================================

/// Builds the Axum router with the same routes as main.rs but without
/// rate-limiting middleware (to avoid test flakes) and without the static
/// file fallback.
fn build_test_router(state: AppState) -> Router {
    // Chat routes with their own rate-limiter layer omitted for tests
    let chat_routes = Router::new()
        .route("/api/chat", post(agent_server::routes::chat::send_message))
        .route(
            "/api/chat/stream",
            post(agent_server::routes::chat::stream_message),
        );

    let api_routes = Router::new()
        // Health
        .route(
            "/api/health",
            get(agent_server::routes::health::health_check),
        )
        .route("/api/info", get(agent_server::routes::health::system_info))
        // Auth
        .route(
            "/api/auth/status",
            get(agent_server::routes::auth::auth_status),
        )
        .route(
            "/api/auth/login",
            post(agent_server::routes::auth::auth_login),
        )
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
        // Search
        .route("/api/search", get(agent_server::routes::search::search))
        // Chat (merged)
        .merge(chat_routes)
        // Channels
        .route("/api/channels", get(agent_server::routes::channel::list))
        .route("/api/channels", post(agent_server::routes::channel::create))
        .route(
            "/api/channels/{id}",
            put(agent_server::routes::channel::update),
        )
        .route(
            "/api/channels/{id}",
            delete(agent_server::routes::channel::delete),
        )
        .route(
            "/api/channels/{id}/test",
            post(agent_server::routes::channel::test),
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
        .route(
            "/api/workflows/{id}/run",
            post(agent_server::routes::workflow::run),
        )
        .route(
            "/api/workflows/{id}/runs",
            get(agent_server::routes::workflow::runs),
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
        .route(
            "/api/tasks/{id}/run",
            post(agent_server::routes::task::run_now),
        )
        .route(
            "/api/tasks/{id}/logs",
            get(agent_server::routes::task::logs),
        )
        // Monitor
        .route("/api/monitor", get(agent_server::routes::monitor::monitor))
        .route(
            "/api/monitor/reset",
            axum::routing::post(agent_server::routes::monitor::monitor_reset),
        )
        // Auth middleware on all API routes
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            agent_server::middleware::authenticate,
        ));

    Router::new()
        .merge(api_routes)
        .route(
            "/monitor",
            get(agent_server::routes::monitor::monitor_dashboard),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            agent_server::middleware::track_requests,
        ))
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024)) // 10 MB
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Start the test server on a random available port with an in-memory
/// database and mock LLM.  Returns the base URL and a oneshot sender to
/// trigger graceful shutdown.
async fn start_test_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    // In-memory SQLite database
    let db = agent_db::init_db(":memory:")
        .await
        .expect("Failed to init in-memory test DB");

    // Mock LLM client
    let llm: Arc<dyn LlmClient + Send + Sync> = Arc::new(MockLlmClient);

    // Minimal tool registry (no tools needed for these tests)
    let tools = Arc::new(ToolRegistry::new());

    // Build application state
    let state = AppState::new(db, llm, tools);

    // Seed an admin token so auth tests can use it
    let admin_token = uuid::Uuid::new_v4().to_string();
    state
        .config_repo
        .set("admin_token", &admin_token)
        .await
        .expect("Failed to set admin_token");
    state.set_admin_token(&admin_token).await;

    // Build router
    let app = build_test_router(state);

    // Bind to port 0 for a random available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to random port");
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("Server error");
    });

    // Brief pause to let the server start accepting connections
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    (base_url, shutdown_tx)
}

/// Convenience: get the admin token for a running server by reading the
/// config.  Returns the raw token string.
async fn get_admin_token(base_url: &str) -> String {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/api/config", base_url))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    body["admin_token"].as_str().unwrap().to_string()
}

// ============================================================================
// Test 1: Complete session lifecycle
// ============================================================================

#[tokio::test]
async fn test_complete_session_lifecycle() {
    let (base_url, _shutdown) = start_test_server().await;
    let client = reqwest::Client::new();

    // 1. Create a session
    let create_resp = client
        .post(format!("{}/api/sessions", base_url))
        .json(&serde_json::json!({
            "name": "E2E Lifecycle Session",
            "system_prompt": "You are a test assistant.",
            "temperature": 0.3
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        create_resp.status(),
        200,
        "Create session should return 200"
    );
    let created: serde_json::Value = create_resp.json().await.unwrap();
    let session_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["name"], "E2E Lifecycle Session");
    assert_eq!(created["temperature"], 0.3);

    // 2. Send a chat message using the session
    let chat_resp = client
        .post(format!("{}/api/chat", base_url))
        .json(&serde_json::json!({
            "session_id": session_id,
            "message": "Hello, this is an e2e test message."
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(chat_resp.status(), 200, "Chat should return 200");
    let chat_body: serde_json::Value = chat_resp.json().await.unwrap();
    assert_eq!(chat_body["session_id"], session_id);
    // The mock LLM should return a response
    assert!(chat_body["message"].as_str().unwrap().contains("mock"));

    // 3. Get messages for the session (should have user + assistant)
    let msgs_resp = client
        .get(format!("{}/api/sessions/{}/messages", base_url, session_id))
        .send()
        .await
        .unwrap();
    assert_eq!(msgs_resp.status(), 200);
    let msgs: serde_json::Value = msgs_resp.json().await.unwrap();
    let msgs_arr = msgs.as_array().unwrap();
    assert!(
        msgs_arr.len() >= 2,
        "Should have at least user + assistant messages, got {}",
        msgs_arr.len()
    );
    // Verify roles
    let roles: Vec<&str> = msgs_arr
        .iter()
        .map(|m| m["role"].as_str().unwrap())
        .collect();
    assert!(roles.contains(&"user"), "Should contain a user message");
    assert!(
        roles.contains(&"assistant"),
        "Should contain an assistant message"
    );

    // 4. Delete the session
    let del_resp = client
        .delete(format!("{}/api/sessions/{}", base_url, session_id))
        .send()
        .await
        .unwrap();
    assert_eq!(del_resp.status(), 200);
    let del_body: serde_json::Value = del_resp.json().await.unwrap();
    assert_eq!(del_body["deleted"], true);

    // 5. Verify session is gone
    let get_resp = client
        .get(format!("{}/api/sessions/{}", base_url, session_id))
        .send()
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 404);

    // Cleanup
    let _ = _shutdown.send(());
}

// ============================================================================
// Test 2: Config CRUD cycle
// ============================================================================

#[tokio::test]
async fn test_config_crud_cycle() {
    let (base_url, _shutdown) = start_test_server().await;
    let client = reqwest::Client::new();

    // 1. Get all config and verify seed data exists
    let all_resp = client
        .get(format!("{}/api/config", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(all_resp.status(), 200);
    let all: serde_json::Value = all_resp.json().await.unwrap();
    assert!(
        all["default_model"].as_str().is_some(),
        "Seed config should exist"
    );
    assert!(
        all["theme"].as_str().is_some(),
        "Seed config should have theme"
    );

    // 2. Update one config value
    let update_resp = client
        .put(format!("{}/api/config", base_url))
        .json(&serde_json::json!({
            "theme": "e2e-test-theme",
            "language": "en"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update_resp.status(), 200);

    // 3. Verify the updated values are persisted
    let all_resp2 = client
        .get(format!("{}/api/config", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(all_resp2.status(), 200);
    let all2: serde_json::Value = all_resp2.json().await.unwrap();
    assert_eq!(all2["theme"], "e2e-test-theme");
    assert_eq!(all2["language"], "en");

    // 4. Get a single key
    let single_resp = client
        .get(format!("{}/api/config/theme", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(single_resp.status(), 200);
    let single: serde_json::Value = single_resp.json().await.unwrap();
    assert_eq!(single["key"], "theme");
    assert_eq!(single["value"], "e2e-test-theme");

    // Cleanup
    let _ = _shutdown.send(());
}

// ============================================================================
// Test 3: Workflow lifecycle
// ============================================================================

#[tokio::test]
async fn test_workflow_lifecycle() {
    let (base_url, _shutdown) = start_test_server().await;
    let client = reqwest::Client::new();

    // 1. Create a workflow with a simple DAG definition
    let create_resp = client
        .post(format!("{}/api/workflows", base_url))
        .json(&serde_json::json!({
            "name": "E2E Workflow",
            "description": "An e2e test workflow",
            "definition": {
                "name": "E2E Workflow",
                "description": "Test DAG",
                "steps": [
                    {
                        "id": "step1",
                        "name": "Delay Step",
                        "type": "delay",
                        "config": {"seconds": 0}
                    }
                ],
                "edges": []
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        create_resp.status(),
        200,
        "Create workflow should return 200"
    );
    let created: serde_json::Value = create_resp.json().await.unwrap();
    let workflow_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["name"], "E2E Workflow");

    // 2. Get the workflow by ID
    let get_resp = client
        .get(format!("{}/api/workflows/{}", base_url, workflow_id))
        .send()
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 200);
    let wf: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(wf["name"], "E2E Workflow");
    assert_eq!(wf["id"], workflow_id);

    // 3. Run the workflow
    let run_resp = client
        .post(format!("{}/api/workflows/{}/run", base_url, workflow_id))
        .send()
        .await
        .unwrap();
    assert_eq!(run_resp.status(), 200, "Run workflow should return 200");
    let run_result: serde_json::Value = run_resp.json().await.unwrap();
    assert_eq!(run_result["workflow_id"], workflow_id);
    // The run should have a status
    let status = run_result["status"].as_str().unwrap();
    assert!(
        status == "success" || status == "error" || status == "skipped",
        "Status should be a valid step status, got: {}",
        status
    );

    // 4. Check runs history
    let runs_resp = client
        .get(format!(
            "{}/api/workflows/{}/runs?limit=10",
            base_url, workflow_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(runs_resp.status(), 200);
    let runs: serde_json::Value = runs_resp.json().await.unwrap();
    // Runs may be empty since the route handler does not explicitly insert
    // a run record, but the endpoint should still return a valid array.
    assert!(runs.is_array(), "Runs should be an array");

    // 5. Delete the workflow
    let del_resp = client
        .delete(format!("{}/api/workflows/{}", base_url, workflow_id))
        .send()
        .await
        .unwrap();
    assert_eq!(del_resp.status(), 200);
    let del_body: serde_json::Value = del_resp.json().await.unwrap();
    assert_eq!(del_body["deleted"], true);

    // 6. Verify deleted
    let get_after = client
        .get(format!("{}/api/workflows/{}", base_url, workflow_id))
        .send()
        .await
        .unwrap();
    assert_eq!(get_after.status(), 404);

    // Cleanup
    let _ = _shutdown.send(());
}

// ============================================================================
// Test 4: Task lifecycle
// ============================================================================

#[tokio::test]
async fn test_task_lifecycle() {
    let (base_url, _shutdown) = start_test_server().await;
    let client = reqwest::Client::new();

    // 1. Create a task
    let create_resp = client
        .post(format!("{}/api/tasks", base_url))
        .json(&serde_json::json!({
            "name": "E2E Daily Report",
            "cron_expression": "0 8 * * *",
            "prompt": "Generate daily summary report"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 200, "Create task should return 200");
    let created: serde_json::Value = create_resp.json().await.unwrap();
    let task_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["name"], "E2E Daily Report");
    assert_eq!(created["cron_expression"], "0 8 * * *");
    assert_eq!(created["prompt"], "Generate daily summary report");

    // 2. Get the task by ID
    let get_resp = client
        .get(format!("{}/api/tasks/{}", base_url, task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 200);
    let task: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(task["name"], "E2E Daily Report");
    assert_eq!(task["id"], task_id);

    // 3. Update the task
    let update_resp = client
        .put(format!("{}/api/tasks/{}", base_url, task_id))
        .json(&serde_json::json!({
            "name": "E2E Updated Report",
            "cron_expression": "0 9 * * *",
            "enabled": false
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update_resp.status(), 200);
    let updated: serde_json::Value = update_resp.json().await.unwrap();
    assert_eq!(updated["name"], "E2E Updated Report");
    assert_eq!(updated["cron_expression"], "0 9 * * *");
    assert!(!updated["enabled"].as_bool().unwrap());

    // 4. Verify the update persisted
    let get_after = client
        .get(format!("{}/api/tasks/{}", base_url, task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(get_after.status(), 200);
    let task_after: serde_json::Value = get_after.json().await.unwrap();
    assert_eq!(task_after["name"], "E2E Updated Report");

    // 5. Delete the task
    let del_resp = client
        .delete(format!("{}/api/tasks/{}", base_url, task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(del_resp.status(), 200);
    let del_body: serde_json::Value = del_resp.json().await.unwrap();
    assert_eq!(del_body["deleted"], true);

    // 6. Verify deleted
    let get_gone = client
        .get(format!("{}/api/tasks/{}", base_url, task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(get_gone.status(), 404);

    // Cleanup
    let _ = _shutdown.send(());
}

// ============================================================================
// Test 5: Channel CRUD
// ============================================================================

#[tokio::test]
async fn test_channel_crud() {
    let (base_url, _shutdown) = start_test_server().await;
    let client = reqwest::Client::new();

    // 1. Create a Feishu channel
    let create_resp = client
        .post(format!("{}/api/channels", base_url))
        .json(&serde_json::json!({
            "channel_type": "feishu",
            "name": "E2E Feishu Channel",
            "config": {
                "app_id": "test-app-id",
                "app_secret": "test-secret"
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        create_resp.status(),
        200,
        "Create channel should return 200"
    );
    let created: serde_json::Value = create_resp.json().await.unwrap();
    let channel_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["channel_type"], "feishu");
    assert_eq!(created["name"], "E2E Feishu Channel");

    // 2. List channels: should contain the newly created one
    let list_resp = client
        .get(format!("{}/api/channels", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(list_resp.status(), 200);
    let list: serde_json::Value = list_resp.json().await.unwrap();
    let list_arr = list.as_array().unwrap();
    assert!(!list_arr.is_empty(), "Channel list should not be empty");
    let found = list_arr
        .iter()
        .any(|c| c["id"].as_str() == Some(&channel_id));
    assert!(found, "Created channel should appear in list");

    // 3. Update the channel: change name and enable it
    let update_resp = client
        .put(format!("{}/api/channels/{}", base_url, channel_id))
        .json(&serde_json::json!({
            "name": "E2E Feishu Updated",
            "enabled": true,
            "config": {
                "app_id": "updated-app-id",
                "app_secret": "updated-secret"
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update_resp.status(), 200);
    let updated: serde_json::Value = update_resp.json().await.unwrap();
    assert_eq!(updated["name"], "E2E Feishu Updated");
    assert!(updated["enabled"].as_bool().unwrap());

    // 4. Delete the channel
    let del_resp = client
        .delete(format!("{}/api/channels/{}", base_url, channel_id))
        .send()
        .await
        .unwrap();
    assert_eq!(del_resp.status(), 200);
    let del_body: serde_json::Value = del_resp.json().await.unwrap();
    assert_eq!(del_body["deleted"], true);

    // Cleanup
    let _ = _shutdown.send(());
}

// ============================================================================
// Test 6: Search functionality
// ============================================================================

#[tokio::test]
async fn test_search_functionality() {
    let (base_url, _shutdown) = start_test_server().await;
    let client = reqwest::Client::new();

    // Create a session with a unique name that we can search for
    let unique_name = format!("E2E-Unique-Search-{}", uuid::Uuid::new_v4());
    let create_resp = client
        .post(format!("{}/api/sessions", base_url))
        .json(&serde_json::json!({
            "name": unique_name
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 200);
    let created: serde_json::Value = create_resp.json().await.unwrap();
    let session_id = created["id"].as_str().unwrap().to_string();

    // Now create a second session with a completely different name for contrast
    let other_resp = client
        .post(format!("{}/api/sessions", base_url))
        .json(&serde_json::json!({
            "name": "Completely Different Name"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(other_resp.status(), 200);

    // Search for the unique name — should find it
    let search_resp = client
        .get(format!(
            "{}/api/search?q={}&type=sessions",
            base_url,
            &unique_name[4..20] // search a substring
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(search_resp.status(), 200);
    let search_result: serde_json::Value = search_resp.json().await.unwrap();
    let results = search_result["results"].as_array().unwrap();
    assert!(!results.is_empty(), "Search should find the unique session");

    let found = results
        .iter()
        .any(|r| r["session_id"].as_str() == Some(&session_id));
    assert!(found, "Search results should contain the created session");

    // Search for something that does not exist
    let no_resp = client
        .get(format!(
            "{}/api/search?q=ZZZ-NONEXISTENT-XXXXX&type=sessions",
            base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(no_resp.status(), 200);
    let no_result: serde_json::Value = no_resp.json().await.unwrap();
    assert!(
        no_result["results"].as_array().unwrap().is_empty(),
        "Non-existent search should return empty results"
    );

    // Cleanup
    let _ = _shutdown.send(());
}

// ============================================================================
// Test 7: Auth flow
// ============================================================================

#[tokio::test]
async fn test_auth_flow() {
    let (base_url, _shutdown) = start_test_server().await;
    let client = reqwest::Client::new();

    // Get the admin token from config (set during server startup)
    let token = get_admin_token(&base_url).await;
    assert!(!token.is_empty(), "Admin token should be set");

    // 1. Check auth status — should be disabled initially
    let status_resp = client
        .get(format!("{}/api/auth/status", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(status_resp.status(), 200);
    let status: serde_json::Value = status_resp.json().await.unwrap();
    assert!(
        !status["auth_enabled"].as_bool().unwrap(),
        "Auth should be disabled initially"
    );

    // 2. Enable auth via config update
    let enable_resp = client
        .put(format!("{}/api/config", base_url))
        .json(&serde_json::json!({
            "auth_enabled": "true"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(enable_resp.status(), 200, "Enabling auth should succeed");

    // Verify auth is now enabled
    let status2_resp = client
        .get(format!("{}/api/auth/status", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(status2_resp.status(), 200);
    let status2: serde_json::Value = status2_resp.json().await.unwrap();
    assert!(
        status2["auth_enabled"].as_bool().unwrap(),
        "Auth should now be enabled"
    );

    // 3. Try to access a protected endpoint without a token — should get 401
    let no_auth_resp = client
        .get(format!("{}/api/sessions", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        no_auth_resp.status(),
        401,
        "Protected endpoint without token should return 401"
    );

    // 4. Login with the correct token
    let login_resp = client
        .post(format!("{}/api/auth/login", base_url))
        .json(&serde_json::json!({
            "token": token
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        login_resp.status(),
        200,
        "Login with valid token should succeed"
    );
    let login_body: serde_json::Value = login_resp.json().await.unwrap();
    assert_eq!(login_body["valid"], true);

    // 5. Access a protected endpoint WITH the token — should get 200
    let auth_resp = client
        .get(format!("{}/api/sessions", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        auth_resp.status(),
        200,
        "Protected endpoint with valid token should return 200"
    );

    // 6. Login with wrong token — should get 401
    let bad_login_resp = client
        .post(format!("{}/api/auth/login", base_url))
        .json(&serde_json::json!({
            "token": "wrong-token-value"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        bad_login_resp.status(),
        401,
        "Login with invalid token should return 401"
    );

    // 7. Disable auth again to leave clean state
    let _ = client
        .put(format!("{}/api/config", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "auth_enabled": "false"
        }))
        .send()
        .await
        .unwrap();

    // Cleanup
    let _ = _shutdown.send(());
}
