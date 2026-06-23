use std::sync::Arc;

use agent_db::repo::{ConfigRepo, MessageRepo, SessionRepo};
use futures::stream::Stream;
use sqlx::SqlitePool;
use tracing::debug;
use uuid::Uuid;

use crate::error::CoreError;
use crate::llm::client::LlmClient;
use crate::llm::types::{ChatMessage, ChatRequest};
use crate::tool::registry::ToolRegistry;

/// Manages conversation sessions and the core agent loop.
///
/// The session manager is responsible for:
///
/// 1. Loading session metadata (system prompt, model, temperature, etc.)
/// 2. Reconstructing the full conversation history from the database
/// 3. Building LLM requests with tool definitions from the `ToolRegistry`
/// 4. Running the agent loop: send request, receive response, handle tool
///    calls, iterate until the LLM returns a final response
/// 5. Saving all messages (user, assistant, tool) to the database
/// 6. Supporting both non-streaming and streaming (SSE) output modes
///
/// The loop is capped at `max_tool_iterations` (default 10) to prevent
/// infinite tool-calling cycles.
pub struct SessionManager {
    llm: Arc<dyn LlmClient>,
    tools: Arc<ToolRegistry>,
    session_repo: SessionRepo,
    message_repo: MessageRepo,
    #[allow(dead_code)]
    config_repo: ConfigRepo,
    max_tool_iterations: usize,
}

impl SessionManager {
    /// Create a new session manager backed by the given LLM client, tool
    /// registry, and database connection pool.
    pub fn new(llm: Arc<dyn LlmClient>, tools: Arc<ToolRegistry>, pool: SqlitePool) -> Self {
        Self {
            llm,
            tools,
            session_repo: SessionRepo::new(pool.clone()),
            message_repo: MessageRepo::new(pool.clone()),
            config_repo: ConfigRepo::new(pool),
            max_tool_iterations: 10,
        }
    }

    /// Process a user message within a session and return the full AI response.
    ///
    /// This is the non-streaming entry point. It loads the session and message
    /// history, runs the agent loop (LLM call -> tool calls -> LLM call -> ...),
    /// saves all messages to the database, and returns the final assistant text.
    #[allow(clippy::too_many_lines)]
    pub async fn process_message(
        &self,
        session_id: &str,
        user_message: &str,
        model: Option<&str>,
    ) -> Result<String, CoreError> {
        let session = self
            .session_repo
            .get(session_id)
            .await?
            .ok_or_else(|| CoreError::SessionNotFound(session_id.to_string()))?;

        let model = model.unwrap_or(&session.model);
        let system_prompt = session.system_prompt.as_deref();

        // Build message list
        let mut messages = Vec::new();

        if let Some(sp) = system_prompt {
            messages.push(ChatMessage {
                role: "system".into(),
                content: sp.to_string(),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        // Load recent messages
        let history = self.message_repo.list_by_session(session_id).await?;

        for msg in &history {
            messages.push(ChatMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
                tool_calls: msg
                    .tool_calls
                    .as_ref()
                    .and_then(|tc| serde_json::from_str(tc).ok()),
                tool_call_id: msg.tool_call_id.clone(),
            });
        }

        // Add user message
        let user_msg_id = Uuid::new_v4().to_string();
        messages.push(ChatMessage {
            role: "user".into(),
            content: user_message.to_string(),
            tool_calls: None,
            tool_call_id: None,
        });

        // Save user message
        self.message_repo
            .insert(&agent_db::models::MessageRow {
                id: user_msg_id,
                session_id: session_id.to_string(),
                role: "user".into(),
                content: user_message.to_string(),
                tool_calls: None,
                tool_call_id: None,
                created_at: String::new(),
            })
            .await?;

        // Agent loop: LLM <-> Tool calls
        let tools_defs = self.tools.get_definitions().await;
        let tools = if tools_defs.is_empty() {
            None
        } else {
            Some(tools_defs.clone())
        }; // clone for reuse

        let mut iteration = 0;
        loop {
            iteration += 1;
            if iteration > self.max_tool_iterations {
                debug!("Max tool iterations reached for session {}", session_id);
                break;
            }

            let request = {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let max_tokens = session.max_tokens as u32;
                ChatRequest {
                    model: model.to_string(),
                    messages: messages.clone(),
                    tools: tools.clone(),
                    temperature: Some(session.temperature),
                    max_tokens: Some(max_tokens),
                    stream: false,
                }
            };

            let response = self.llm.chat(&request).await?;

            let choice = response
                .choices
                .first()
                .ok_or_else(|| CoreError::LlmApi("No choices in response".into()))?;

            let msg = &choice.message;

            // Check if the LLM wants to call tools
            if let Some(ref tool_calls) = msg.tool_calls {
                if tool_calls.is_empty() || choice.finish_reason.as_deref() == Some("stop") {
                    // No more tool calls — final response
                    let assistant_msg_id = Uuid::new_v4().to_string();
                    self.message_repo
                        .insert(&agent_db::models::MessageRow {
                            id: assistant_msg_id,
                            session_id: session_id.to_string(),
                            role: "assistant".into(),
                            content: msg.content.clone(),
                            tool_calls: if tool_calls.is_empty() {
                                None
                            } else {
                                match serde_json::to_string(tool_calls) {
                                    Ok(json) => Some(json),
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to serialize tool_calls for session {}: {}",
                                            session_id,
                                            e
                                        );
                                        None
                                    }
                                }
                            },
                            tool_call_id: None,
                            created_at: String::new(),
                        })
                        .await?;

                    self.session_repo.touch(session_id).await?;

                    return Ok(msg.content.clone());
                }

                // Append assistant message with tool calls
                messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: msg.content.clone(),
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                });

                // Execute each tool call
                for tc in tool_calls {
                    let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)?;
                    let result = self.tools.execute(&tc.function.name, args).await;

                    let tool_content = match result {
                        Ok(r) => r.content,
                        Err(e) => format!("Error: {e}"),
                    };

                    messages.push(ChatMessage {
                        role: "tool".into(),
                        content: tool_content,
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                    });

                    // Save tool message
                    self.message_repo
                        .insert(&agent_db::models::MessageRow {
                            id: Uuid::new_v4().to_string(),
                            session_id: session_id.to_string(),
                            role: "tool".into(),
                            content: format!("Tool: {}", tc.function.name),
                            tool_calls: None,
                            tool_call_id: Some(tc.id.clone()),
                            created_at: String::new(),
                        })
                        .await?;
                }

                // Continue loop — LLM will process tool results
                continue;
            }

            // No tool calls — final assistant response
            let assistant_msg_id = Uuid::new_v4().to_string();
            let tool_calls_json =
                msg.tool_calls
                    .as_ref()
                    .and_then(|tc| match serde_json::to_string(tc) {
                        Ok(json) => Some(json),
                        Err(e) => {
                            tracing::warn!(
                                "Failed to serialize tool_calls for session {}: {}",
                                session_id,
                                e
                            );
                            None
                        }
                    });

            self.message_repo
                .insert(&agent_db::models::MessageRow {
                    id: assistant_msg_id,
                    session_id: session_id.to_string(),
                    role: "assistant".into(),
                    content: msg.content.clone(),
                    tool_calls: tool_calls_json,
                    tool_call_id: None,
                    created_at: String::new(),
                })
                .await?;

            self.session_repo.touch(session_id).await?;

            return Ok(msg.content.clone());
        }

        Err(CoreError::LlmApi("Max tool iterations exceeded".into()))
    }

    /// Process a user message and return an SSE-style event stream.
    ///
    /// Events are JSON objects discriminated by a `"type"` field:
    ///
    /// | Type | Meaning |
    /// |------|---------|
    /// | `"thinking"` | Status update (processing started, tool results received, etc.) |
    /// | `"delta"` | A chunk of response text (approximately 12 words each) |
    /// | `"tool_start"` | A tool invocation has begun (includes `"tool"` and `"args"`) |
    /// | `"tool_end"` | A tool has completed (includes `"tool"` and `"result"`) |
    /// | `"done"` | The stream is complete (includes `"session_id"`) |
    /// | `"error"` | An error occurred (includes `"message"`) |
    #[allow(clippy::too_many_lines)]
    pub fn process_message_stream(
        &self,
        session_id: &str,
        user_message: &str,
        model: Option<&str>,
    ) -> impl Stream<Item = Result<serde_json::Value, CoreError>> + Send + 'static {
        let llm = Arc::clone(&self.llm);
        let tools = Arc::clone(&self.tools);
        let session_repo = self.session_repo.clone();
        let message_repo = self.message_repo.clone();
        let max_tool_iterations = self.max_tool_iterations;

        let session_id_owned = session_id.to_string();
        let user_message_owned = user_message.to_string();
        let model_owned = model.map(ToString::to_string);

        let (tx, rx) = tokio::sync::mpsc::channel::<Result<serde_json::Value, CoreError>>(64);

        tokio::spawn(async move {
            let result = async {
                // Load session
                let session = session_repo
                    .get(&session_id_owned)
                    .await?
                    .ok_or_else(|| CoreError::SessionNotFound(session_id_owned.clone()))?;

                let model = model_owned.as_deref().unwrap_or(&session.model);
                let system_prompt = session.system_prompt.as_deref();

                // Build message list
                let mut messages = Vec::new();
                if let Some(sp) = system_prompt {
                    messages.push(ChatMessage {
                        role: "system".into(),
                        content: sp.to_string(),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }

                let history = message_repo.list_by_session(&session_id_owned).await?;

                for msg in &history {
                    messages.push(ChatMessage {
                        role: msg.role.clone(),
                        content: msg.content.clone(),
                        tool_calls: msg
                            .tool_calls
                            .as_ref()
                            .and_then(|tc| serde_json::from_str(tc).ok()),
                        tool_call_id: msg.tool_call_id.clone(),
                    });
                }

                // Save user message
                messages.push(ChatMessage {
                    role: "user".into(),
                    content: user_message_owned.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                });
                message_repo
                    .insert(&agent_db::models::MessageRow {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session_id_owned.clone(),
                        role: "user".into(),
                        content: user_message_owned.clone(),
                        tool_calls: None,
                        tool_call_id: None,
                        created_at: String::new(),
                    })
                    .await?;

                // Agent loop
                let tools_defs = tools.get_definitions().await;
                let tools_param = if tools_defs.is_empty() {
                    None
                } else {
                    Some(tools_defs.clone())
                };

                let mut iteration = 0;
                loop {
                    iteration += 1;
                    if iteration > max_tool_iterations {
                        let _ = tx
                            .send(Ok(serde_json::json!({
                                "type": "thinking",
                                "content": "Reached maximum tool iteration limit"
                            })))
                            .await;
                        break;
                    }

                    // Initial thinking event
                    if iteration == 1 {
                        let _ = tx
                            .send(Ok(serde_json::json!({
                                "type": "thinking",
                                "content": "Agent is thinking..."
                            })))
                            .await;
                    }

                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let max_tokens = session.max_tokens as u32;
                    let request = ChatRequest {
                        model: model.to_string(),
                        messages: messages.clone(),
                        tools: tools_param.clone(),
                        temperature: Some(session.temperature),
                        max_tokens: Some(max_tokens),
                        stream: false,
                    }; // first stream request uses same cast

                    let response = llm.chat(&request).await?;

                    let choice = response
                        .choices
                        .first()
                        .ok_or_else(|| CoreError::LlmApi("No choices in response".into()))?;
                    let msg = &choice.message;

                    // --- Tool call branch ---
                    if let Some(ref call_groups) = msg.tool_calls {
                        if call_groups.is_empty() || choice.finish_reason.as_deref() == Some("stop")
                        {
                            // Empty tool-calls list or forced stop — treat as final
                            let assistant_msg_id = uuid::Uuid::new_v4().to_string();
                            let tc_json = if call_groups.is_empty() {
                                None
                            } else {
                                match serde_json::to_string(call_groups) {
                                    Ok(json) => Some(json),
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to serialize tool_calls for session {}: {}",
                                            session_id_owned,
                                            e
                                        );
                                        None
                                    }
                                }
                            };

                            message_repo
                                .insert(&agent_db::models::MessageRow {
                                    id: assistant_msg_id,
                                    session_id: session_id_owned.clone(),
                                    role: "assistant".into(),
                                    content: msg.content.clone(),
                                    tool_calls: tc_json,
                                    tool_call_id: None,
                                    created_at: String::new(),
                                })
                                .await?;

                            session_repo.touch(&session_id_owned).await?;

                            // Stream content in chunks
                            for chunk in chunk_text(&msg.content, 12) {
                                let _ = tx
                                    .send(Ok(serde_json::json!({
                                        "type": "delta",
                                        "content": chunk
                                    })))
                                    .await;
                            }

                            let _ = tx
                                .send(Ok(serde_json::json!({
                                    "type": "done",
                                    "session_id": &session_id_owned
                                })))
                                .await;
                            return Ok(());
                        }

                        // Append assistant message with tool calls
                        messages.push(ChatMessage {
                            role: "assistant".into(),
                            content: msg.content.clone(),
                            tool_calls: Some(call_groups.clone()),
                            tool_call_id: None,
                        });

                        // Execute each tool call
                        for tc in call_groups {
                            let _ = tx
                                .send(Ok(serde_json::json!({
                                    "type": "tool_start",
                                    "tool": tc.function.name,
                                    "args": tc.function.arguments
                                })))
                                .await;

                            let args: serde_json::Value =
                                serde_json::from_str(&tc.function.arguments)?;
                            let exec_result = tools.execute(&tc.function.name, args).await;

                            let tool_content = match &exec_result {
                                Ok(r) => r.content.clone(),
                                Err(e) => format!("Error: {e}"),
                            };

                            let _ = tx
                                .send(Ok(serde_json::json!({
                                    "type": "tool_end",
                                    "tool": tc.function.name,
                                    "result": &tool_content
                                })))
                                .await;

                            messages.push(ChatMessage {
                                role: "tool".into(),
                                content: tool_content,
                                tool_calls: None,
                                tool_call_id: Some(tc.id.clone()),
                            });

                            message_repo
                                .insert(&agent_db::models::MessageRow {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    session_id: session_id_owned.clone(),
                                    role: "tool".into(),
                                    content: format!("Tool: {}", tc.function.name),
                                    tool_calls: None,
                                    tool_call_id: Some(tc.id.clone()),
                                    created_at: String::new(),
                                })
                                .await?;
                        }

                        let _ = tx
                            .send(Ok(serde_json::json!({
                                "type": "thinking",
                                "content": "Agent is processing tool results..."
                            })))
                            .await;

                        continue;
                    }

                    // --- No tool calls — final assistant response ---
                    let assistant_msg_id = uuid::Uuid::new_v4().to_string();
                    let tool_calls_json =
                        msg.tool_calls
                            .as_ref()
                            .and_then(|tc| match serde_json::to_string(tc) {
                                Ok(json) => Some(json),
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to serialize tool_calls for session {}: {}",
                                        session_id_owned,
                                        e
                                    );
                                    None
                                }
                            });

                    message_repo
                        .insert(&agent_db::models::MessageRow {
                            id: assistant_msg_id,
                            session_id: session_id_owned.clone(),
                            role: "assistant".into(),
                            content: msg.content.clone(),
                            tool_calls: tool_calls_json,
                            tool_call_id: None,
                            created_at: String::new(),
                        })
                        .await?;

                    session_repo.touch(&session_id_owned).await?;

                    // Stream content in chunks
                    for chunk in chunk_text(&msg.content, 12) {
                        let _ = tx
                            .send(Ok(serde_json::json!({
                                "type": "delta",
                                "content": chunk
                            })))
                            .await;
                    }

                    let _ = tx
                        .send(Ok(serde_json::json!({
                            "type": "done",
                            "session_id": &session_id_owned
                        })))
                        .await;
                    return Ok(());
                }

                Err(CoreError::LlmApi("Max tool iterations exceeded".into()))
            };

            if let Err(e) = result.await {
                let _ = tx
                    .send(Ok(serde_json::json!({
                        "type": "error",
                        "message": e.to_string()
                    })))
                    .await;
            }
        });

        tokio_stream::wrappers::ReceiverStream::new(rx)
    }
}

/// Split text into chunks of approximately `max_words` words each.
fn chunk_text(text: &str, max_words: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_inclusive([' ', '\n']).collect();
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut count = 0;

    for word in words {
        current.push_str(word);
        count += 1;
        if count >= max_words {
            chunks.push(std::mem::take(&mut current));
            count = 0;
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    if chunks.is_empty() {
        // If text has no spaces, still return it as one chunk
        if !text.is_empty() {
            chunks.push(text.to_string());
        }
    }

    chunks
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    // ── chunk_text tests ──

    #[test]
    fn test_chunk_text_empty_string() {
        let chunks = chunk_text("", 12);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_text_single_word() {
        let chunks = chunk_text("hello", 5);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello");
    }

    #[test]
    fn test_chunk_text_no_spaces_long() {
        // A long string without spaces should still be returned as one chunk
        let long = "x".repeat(100);
        let chunks = chunk_text(&long, 12);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], long);
    }

    #[test]
    fn test_chunk_text_exact_words_per_chunk() {
        let text = "one two three four five six";
        let chunks = chunk_text(text, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].trim(), "one two");
        assert_eq!(chunks[1].trim(), "three four");
        assert_eq!(chunks[2].trim(), "five six");
    }

    #[test]
    fn test_chunk_text_uneven_split() {
        let text = "a b c d e";
        let chunks = chunk_text(text, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].trim(), "a b");
        assert_eq!(chunks[1].trim(), "c d");
        assert_eq!(chunks[2].trim(), "e");
    }

    #[test]
    fn test_chunk_text_with_newlines() {
        let text = "line1 word1\nline2 word2\nline3";
        let chunks = chunk_text(text, 3);
        // split_inclusive includes the separator char
        assert!(!chunks.is_empty());
        let joined: String = chunks.iter().map(|s| s.as_str()).collect();
        assert_eq!(joined, text);
    }

    #[test]
    fn test_chunk_text_unicode_and_special_chars() {
        let text = "你好 世界 🌍 🧪 test こんにちは";
        let chunks = chunk_text(text, 2);
        assert!(!chunks.is_empty());
        let joined: String = chunks.iter().map(|s| s.as_str()).collect();
        assert_eq!(joined, text);
    }

    #[test]
    fn test_chunk_text_very_long_message() {
        let words: Vec<String> = (0..5000).map(|i| format!("word{}", i)).collect();
        let text = words.join(" ");
        let chunks = chunk_text(&text, 12);
        assert!(
            chunks.len() > 1,
            "Very long message should produce multiple chunks"
        );
        let full_len: usize = chunks.iter().map(|c| c.len()).sum();
        assert_eq!(full_len, text.len(), "All content should be preserved");
    }

    #[test]
    fn test_chunk_text_single_character_words() {
        let text = "a b c d e f g h i j k l m n o";
        let chunks = chunk_text(text, 5);
        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn test_chunk_text_chunk_size_exceeded_for_no_space_text() {
        // Text with no spaces - should return as single chunk regardless of size
        let long_no_space = "x".repeat(5000);
        let chunks = chunk_text(&long_no_space, 12);
        assert_eq!(
            chunks.len(),
            1,
            "Text without spaces should be a single chunk"
        );
        assert_eq!(chunks[0].len(), 5000);
    }
}
