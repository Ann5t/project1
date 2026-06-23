//! Integration tests for agent-db
//!
//! Run with: cargo test -p agent-db

use agent_db::models::*;
use agent_db::repo::*;
use agent_db::run_migrations;
use sqlx::SqlitePool;

/// Create an in-memory SQLite pool and run all migrations.
async fn setup_in_memory_db() -> SqlitePool {
    let pool = agent_db::pool::create_pool("sqlite::memory:").await
        .expect("Failed to create in-memory pool");

    // Enable WAL and foreign keys for in-memory DB too
    sqlx::query("PRAGMA journal_mode=WAL;")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys=ON;")
        .execute(&pool)
        .await
        .unwrap();

    run_migrations(&pool).await.expect("Migrations failed");
    pool
}

// ============================================================================
// Migration Tests
// ============================================================================

#[tokio::test]
async fn migrations_run_all_tables_created() {
    let pool = setup_in_memory_db().await;

    // Verify each table exists by counting rows (should not error)
    let tables = vec![
        "config",
        "sessions",
        "messages",
        "channels",
        "workflows",
        "workflow_runs",
        "scheduled_tasks",
        "task_logs",
    ];

    for table in &tables {
        let query = format!("SELECT COUNT(*) FROM {}", table);
        let count: (i64,) = sqlx::query_as(&query)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|e| panic!("Table '{}' missing or query failed: {}", table, e));
        // Tables exist (even if empty)
        let _ = count;
    }
}

#[tokio::test]
async fn seed_config_populated() {
    let pool = setup_in_memory_db().await;

    let config_repo = ConfigRepo::new(pool);
    let all = config_repo.get_all().await.unwrap();

    // The seed migration inserts these keys
    assert!(all.contains_key("api_key"), "api_key not seeded");
    assert!(all.contains_key("default_model"), "default_model not seeded");
    assert!(all.contains_key("system_prompt"), "system_prompt not seeded");
    assert!(all.contains_key("theme"), "theme not seeded");
    assert!(all.contains_key("language"), "language not seeded");
    assert!(all.contains_key("temperature"), "temperature not seeded");
    assert!(all.contains_key("max_tokens"), "max_tokens not seeded");
}

#[tokio::test]
async fn seed_config_grouped_by_category() {
    let pool = setup_in_memory_db().await;

    let config_repo = ConfigRepo::new(pool);
    let grouped = config_repo.get_all_grouped().await.unwrap();

    assert!(grouped.contains_key("llm"), "llm category missing");
    assert!(grouped.contains_key("ui"), "ui category missing");
    assert!(grouped.contains_key("system"), "system category missing");

    let llm_config = &grouped["llm"];
    assert!(llm_config.contains_key("api_key"));
    assert!(llm_config.contains_key("base_url"));
}

// ============================================================================
// ConfigRepo Tests
// ============================================================================

#[tokio::test]
async fn config_get_and_set() {
    let pool = setup_in_memory_db().await;
    let repo = ConfigRepo::new(pool);

    // Get non-existent key
    let val = repo.get("nonexistent").await.unwrap();
    assert!(val.is_none());

    // Set a key
    repo.set("test_key", "test_value").await.unwrap();

    // Get the key back
    let val = repo.get("test_key").await.unwrap();
    assert_eq!(val, Some("test_value".to_string()));

    // Update the key
    repo.set("test_key", "updated_value").await.unwrap();
    let val = repo.get("test_key").await.unwrap();
    assert_eq!(val, Some("updated_value".to_string()));
}

#[tokio::test]
async fn config_get_or_default() {
    let pool = setup_in_memory_db().await;
    let repo = ConfigRepo::new(pool);

    // Returns default when key is missing
    let val = repo.get_or_default("missing", "default_val").await.unwrap();
    assert_eq!(val, "default_val");

    // Returns value when key exists
    repo.set("present", "real_val").await.unwrap();
    let val = repo.get_or_default("present", "default_val").await.unwrap();
    assert_eq!(val, "real_val");
}

#[tokio::test]
async fn config_get_all_returns_flat_map() {
    let pool = setup_in_memory_db().await;
    let repo = ConfigRepo::new(pool);

    repo.set("custom_key", "custom_value").await.unwrap();

    let all = repo.get_all().await.unwrap();
    assert!(all.contains_key("custom_key"));
    assert_eq!(all["custom_key"], "custom_value");
    // Seed data should also be present
    assert!(all.len() > 1);
}

#[tokio::test]
async fn config_update_all_batch() {
    let pool = setup_in_memory_db().await;
    let repo = ConfigRepo::new(pool);

    let mut batch = std::collections::HashMap::new();
    batch.insert("b1".to_string(), "v1".to_string());
    batch.insert("b2".to_string(), "v2".to_string());

    repo.update_all(&batch).await.unwrap();

    assert_eq!(repo.get("b1").await.unwrap(), Some("v1".to_string()));
    assert_eq!(repo.get("b2").await.unwrap(), Some("v2".to_string()));
}

#[tokio::test]
async fn config_delete() {
    let pool = setup_in_memory_db().await;
    let repo = ConfigRepo::new(pool);

    repo.set("to_delete", "value").await.unwrap();
    assert!(repo.get("to_delete").await.unwrap().is_some());

    repo.delete("to_delete").await.unwrap();
    assert!(repo.get("to_delete").await.unwrap().is_none());
}

#[tokio::test]
async fn config_delete_nonexistent_ok() {
    let pool = setup_in_memory_db().await;
    let repo = ConfigRepo::new(pool);

    // Deleting a non-existent key should not error
    repo.delete("never_existed").await.unwrap();
}

// ============================================================================
// SessionRepo Tests
// ============================================================================

fn make_session(id: &str, name: &str) -> SessionRow {
    SessionRow {
        id: id.to_string(),
        name: name.to_string(),
        agent_id: None,
        system_prompt: None,
        model: "deepseek-chat".to_string(),
        temperature: 0.7,
        max_tokens: 4096,
        channel: "web".to_string(),
        channel_chat_id: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

#[tokio::test]
async fn session_create_and_get() {
    let pool = setup_in_memory_db().await;
    let repo = SessionRepo::new(pool);

    let session = make_session("sess-1", "Test Session");
    repo.create(&session).await.unwrap();

    let fetched = repo.get("sess-1").await.unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.id, "sess-1");
    assert_eq!(fetched.name, "Test Session");
    assert_eq!(fetched.model, "deepseek-chat");
    assert_eq!(fetched.temperature, 0.7);
    assert_eq!(fetched.max_tokens, 4096);
    assert_eq!(fetched.channel, "web");
    // created_at and updated_at should be auto-populated
    assert!(!fetched.created_at.is_empty());
    assert!(!fetched.updated_at.is_empty());
}

#[tokio::test]
async fn session_list_multiple() {
    let pool = setup_in_memory_db().await;
    let repo = SessionRepo::new(pool);

    repo.create(&make_session("s1", "First")).await.unwrap();
    repo.create(&make_session("s2", "Second")).await.unwrap();
    repo.create(&make_session("s3", "Third")).await.unwrap();

    let sessions = repo.list().await.unwrap();
    assert_eq!(sessions.len(), 3);

    // Should be ordered by updated_at DESC (newest first since all created together,
    // ordering might vary slightly — just check all are present)
    let names: Vec<&str> = sessions.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"First"));
    assert!(names.contains(&"Second"));
    assert!(names.contains(&"Third"));
}

#[tokio::test]
async fn session_get_nonexistent() {
    let pool = setup_in_memory_db().await;
    let repo = SessionRepo::new(pool);

    let fetched = repo.get("nonexistent").await.unwrap();
    assert!(fetched.is_none());
}

#[tokio::test]
async fn session_update_fields() {
    let pool = setup_in_memory_db().await;
    let repo = SessionRepo::new(pool);

    let mut session = make_session("sess-upd", "Original");
    repo.create(&session).await.unwrap();

    session.name = "Updated Name".to_string();
    session.temperature = 0.3;
    session.max_tokens = 1024;
    session.channel = "feishu".to_string();
    repo.update(&session).await.unwrap();

    let fetched = repo.get("sess-upd").await.unwrap().unwrap();
    assert_eq!(fetched.name, "Updated Name");
    assert_eq!(fetched.temperature, 0.3);
    assert_eq!(fetched.max_tokens, 1024);
    assert_eq!(fetched.channel, "feishu");
}

#[tokio::test]
async fn session_touch_updates_timestamp() {
    let pool = setup_in_memory_db().await;
    let repo = SessionRepo::new(pool);

    repo.create(&make_session("sess-touch", "Touch Me")).await.unwrap();
    let before = repo.get("sess-touch").await.unwrap().unwrap().updated_at.clone();

    // Small sleep to ensure timestamp actually changes
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    repo.touch("sess-touch").await.unwrap();

    let after = repo.get("sess-touch").await.unwrap().unwrap().updated_at;
    assert_ne!(before, after, "Timestamp should change after touch");
}

#[tokio::test]
async fn session_delete() {
    let pool = setup_in_memory_db().await;
    let repo = SessionRepo::new(pool);

    repo.create(&make_session("sess-del", "Delete Me")).await.unwrap();
    assert!(repo.get("sess-del").await.unwrap().is_some());

    repo.delete("sess-del").await.unwrap();
    assert!(repo.get("sess-del").await.unwrap().is_none());
}

#[tokio::test]
async fn session_with_optional_fields() {
    let pool = setup_in_memory_db().await;
    let repo = SessionRepo::new(pool);

    let session = SessionRow {
        id: "sess-opt".to_string(),
        name: "With Options".to_string(),
        agent_id: Some("agent-1".to_string()),
        system_prompt: Some("You are a helpful assistant.".to_string()),
        model: "deepseek-reasoner".to_string(),
        temperature: 0.2,
        max_tokens: 8192,
        channel: "qq".to_string(),
        channel_chat_id: Some("chat-12345".to_string()),
        created_at: String::new(),
        updated_at: String::new(),
    };

    repo.create(&session).await.unwrap();
    let fetched = repo.get("sess-opt").await.unwrap().unwrap();

    assert_eq!(fetched.agent_id.as_deref(), Some("agent-1"));
    assert_eq!(
        fetched.system_prompt.as_deref(),
        Some("You are a helpful assistant.")
    );
    assert_eq!(fetched.channel_chat_id.as_deref(), Some("chat-12345"));
}

// ============================================================================
// MessageRepo Tests
// ============================================================================

fn make_message(id: &str, session_id: &str, role: &str, content: &str) -> MessageRow {
    MessageRow {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        tool_calls: None,
        tool_call_id: None,
        created_at: String::new(),
    }
}

#[tokio::test]
async fn message_insert_and_list() {
    let pool = setup_in_memory_db().await;
    let session_repo = SessionRepo::new(pool.clone());
    let message_repo = MessageRepo::new(pool);

    // Create a parent session first
    session_repo.create(&make_session("sess-msg", "Msg Test")).await.unwrap();

    message_repo.insert(&make_message("m1", "sess-msg", "user", "Hello")).await.unwrap();
    message_repo.insert(&make_message("m2", "sess-msg", "assistant", "Hi!")).await.unwrap();

    let messages = message_repo.list_by_session("sess-msg").await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "Hello");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "Hi!");
}

#[tokio::test]
async fn message_list_by_session_ordered_asc() {
    let pool = setup_in_memory_db().await;
    let session_repo = SessionRepo::new(pool.clone());
    let message_repo = MessageRepo::new(pool);

    session_repo.create(&make_session("sess-ordered", "Ordered")).await.unwrap();

    message_repo.insert(&make_message("ma", "sess-ordered", "user", "A")).await.unwrap();
    message_repo.insert(&make_message("mb", "sess-ordered", "assistant", "B")).await.unwrap();
    message_repo.insert(&make_message("mc", "sess-ordered", "user", "C")).await.unwrap();

    let messages = message_repo.list_by_session("sess-ordered").await.unwrap();
    // Should be ordered by created_at ASC
    let contents: Vec<&str> = messages.iter().map(|m| m.content.as_str()).collect();
    assert_eq!(contents, vec!["A", "B", "C"]);
}

#[tokio::test]
async fn message_list_recent() {
    let pool = setup_in_memory_db().await;
    let session_repo = SessionRepo::new(pool.clone());
    let message_repo = MessageRepo::new(pool);

    session_repo.create(&make_session("sess-recent", "Recent")).await.unwrap();

    for i in 0..10 {
        message_repo
            .insert(&make_message(
                &format!("rm{}", i),
                "sess-recent",
                "user",
                &format!("Message {}", i),
            ))
            .await
            .unwrap();
    }

    let recent = message_repo.list_recent("sess-recent", 3).await.unwrap();
    assert_eq!(recent.len(), 3);
    // Most recent first (DESC)
    assert!(recent[0].content.contains("Message 9"));
    assert!(recent[1].content.contains("Message 8"));
    assert!(recent[2].content.contains("Message 7"));
}

#[tokio::test]
async fn message_delete_by_session() {
    let pool = setup_in_memory_db().await;
    let session_repo = SessionRepo::new(pool.clone());
    let message_repo = MessageRepo::new(pool);

    session_repo.create(&make_session("sess-del-msg", "Del Msg")).await.unwrap();
    message_repo.insert(&make_message("m1", "sess-del-msg", "user", "x")).await.unwrap();
    message_repo.insert(&make_message("m2", "sess-del-msg", "assistant", "y")).await.unwrap();

    assert_eq!(message_repo.list_by_session("sess-del-msg").await.unwrap().len(), 2);

    message_repo.delete_by_session("sess-del-msg").await.unwrap();
    assert!(message_repo.list_by_session("sess-del-msg").await.unwrap().is_empty());
}

#[tokio::test]
async fn message_empty_session() {
    let pool = setup_in_memory_db().await;
    let session_repo = SessionRepo::new(pool.clone());
    let message_repo = MessageRepo::new(pool);

    session_repo.create(&make_session("sess-empty", "Empty")).await.unwrap();

    let messages = message_repo.list_by_session("sess-empty").await.unwrap();
    assert!(messages.is_empty());
}

#[tokio::test]
async fn message_with_tool_calls() {
    let pool = setup_in_memory_db().await;
    let session_repo = SessionRepo::new(pool.clone());
    let message_repo = MessageRepo::new(pool);

    session_repo.create(&make_session("sess-tool", "Tool")).await.unwrap();

    let msg = MessageRow {
        id: "tool-msg-1".to_string(),
        session_id: "sess-tool".to_string(),
        role: "assistant".to_string(),
        content: "Let me calculate that.".to_string(),
        tool_calls: Some(r#"[{"id":"call_1","type":"function","function":{"name":"calculator","arguments":"{\"expression\":\"2+2\"}"}}]"#.to_string()),
        tool_call_id: None,
        created_at: String::new(),
    };

    message_repo.insert(&msg).await.unwrap();
    let messages = message_repo.list_by_session("sess-tool").await.unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].tool_calls.is_some());
    assert!(messages[0].tool_calls.as_ref().unwrap().contains("calculator"));
}

#[tokio::test]
async fn message_cascade_on_session_delete() {
    let pool = setup_in_memory_db().await;
    let session_repo = SessionRepo::new(pool.clone());
    let message_repo = MessageRepo::new(pool);

    session_repo.create(&make_session("sess-cascade", "Cascade")).await.unwrap();
    message_repo.insert(&make_message("mx", "sess-cascade", "user", "test")).await.unwrap();

    // Delete the session — messages should be cascade-deleted
    session_repo.delete("sess-cascade").await.unwrap();
    let messages = message_repo.list_by_session("sess-cascade").await.unwrap();
    assert!(messages.is_empty());
}

// ============================================================================
// Cross-repo integration: session + messages
// ============================================================================

#[tokio::test]
async fn full_session_lifecycle_with_messages() {
    let pool = setup_in_memory_db().await;
    let config_repo = ConfigRepo::new(pool.clone());
    let session_repo = SessionRepo::new(pool.clone());
    let message_repo = MessageRepo::new(pool);

    // Create a session
    let session = SessionRow {
        id: "lifecycle-1".to_string(),
        name: "Lifecycle Test".to_string(),
        agent_id: None,
        system_prompt: Some("Be concise.".to_string()),
        model: "deepseek-chat".to_string(),
        temperature: 0.5,
        max_tokens: 2048,
        channel: "web".to_string(),
        channel_chat_id: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    session_repo.create(&session).await.unwrap();

    // Add messages
    message_repo.insert(&make_message("msg-1", "lifecycle-1", "system", "Be concise.")).await.unwrap();
    message_repo.insert(&make_message("msg-2", "lifecycle-1", "user", "Hi")).await.unwrap();
    message_repo.insert(&make_message("msg-3", "lifecycle-1", "assistant", "Hello!")).await.unwrap();

    // Verify messages
    let msgs = message_repo.list_by_session("lifecycle-1").await.unwrap();
    assert_eq!(msgs.len(), 3);

    // Verify session exists
    assert!(session_repo.get("lifecycle-1").await.unwrap().is_some());

    // Verify config has seed data
    let all_config = config_repo.get_all().await.unwrap();
    assert!(all_config.len() > 0, "Config should have seed data");

    // Clean up
    message_repo.delete_by_session("lifecycle-1").await.unwrap();
    session_repo.delete("lifecycle-1").await.unwrap();

    assert!(session_repo.get("lifecycle-1").await.unwrap().is_none());
    assert!(message_repo.list_by_session("lifecycle-1").await.unwrap().is_empty());
}
