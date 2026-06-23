use async_trait::async_trait;
use axum::response::sse::{Event as SseEvent, Sse};
use futures::stream::Stream;
use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

use super::stats;
use super::types::{ChatRequest, ChatResponse, StreamDelta, ToolCall};
use crate::error::CoreError;

/// Abstract LLM client trait.
///
/// Implement this trait for any LLM provider (DeepSeek, OpenAI, Anthropic, etc.).
/// The trait requires three methods:
///
/// - `chat`: Non-streaming chat completion.
/// - `chat_stream`: Streaming chat completion yielding content deltas.
/// - `list_models`: List available model IDs from the API.
///
/// All methods are async and return `Result<T, CoreError>`.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a non-streaming chat completion request.
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, CoreError>;

    /// List available models from the API.
    async fn list_models(&self) -> Result<Vec<String>, CoreError>;

    /// Send a streaming chat completion request.
    ///
    /// Each yielded item is a delta content string from the LLM.
    fn chat_stream(
        &self,
        request: &ChatRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<String, CoreError>> + Send>>;
}

/// DeepSeek API client implementing the OpenAI-compatible chat completions API.
///
/// # Example
///
/// ```no_run
/// use agent_core::llm::client::DeepSeekClient;
/// let client = DeepSeekClient::new(
///     "sk-xxx".into(),
///     None,  // default base_url: https://api.deepseek.com/v1
///     None,  // default model: deepseek-chat
/// );
/// ```
pub struct DeepSeekClient {
    client: Client,
    base_url: String,
    api_key: String,
    default_model: String,
    /// Cancellation signal for in-flight streaming requests.
    /// Call `notify_waiters()` on this to gracefully abort all
    /// active SSE streams (e.g. during server shutdown).
    pub shutdown_notify: Arc<tokio::sync::Notify>,
}

impl DeepSeekClient {
    /// Create a new DeepSeek API client.
    ///
    /// # Arguments
    ///
    /// * `api_key` - DeepSeek API key (from platform.deepseek.com).
    /// * `base_url` - Optional API base URL (defaults to `https://api.deepseek.com/v1`).
    /// * `default_model` - Optional default model name (defaults to `deepseek-chat`).
    pub fn new(api_key: String, base_url: Option<String>, default_model: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_else(|e| {
                // Fallback: build with default settings if custom config fails.
                // This is extremely rare — only happens on TLS backend init failure.
                tracing::warn!("Failed to build reqwest client with custom settings: {e}. Falling back to default client.");
                Client::new()
            });

        Self {
            client,
            base_url: base_url.unwrap_or_else(|| "https://api.deepseek.com/v1".into()),
            api_key,
            default_model: default_model.unwrap_or_else(|| "deepseek-chat".into()),
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Return the default model name for this client.
    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    /// Signal all in-flight streaming requests to abort gracefully.
    ///
    /// Call this during server shutdown so that background tasks spawned by
    /// [`chat_stream_inner`] exit their event loop instead of lingering.
    pub fn cancel_pending_streams(&self) {
        self.shutdown_notify.notify_waiters();
    }

    /// Stream a chat completion and return an Axum SSE response.
    ///
    /// Each SSE event carries JSON:
    /// ```json
    /// {"delta": "...", "finish_reason": "stop"}
    /// ```
    ///
    /// On error, the SSE event includes `"error"` instead of `"finish_reason"`.
    #[allow(clippy::type_complexity)]
    pub fn chat_stream_sse(
        &self,
        request: &ChatRequest,
    ) -> Sse<Pin<Box<dyn Stream<Item = Result<SseEvent, Infallible>> + Send>>> {
        let inner = self.chat_stream_inner(request);
        let sse_stream: Pin<Box<dyn Stream<Item = Result<SseEvent, Infallible>> + Send>> =
            Box::pin(inner.map(|item| {
                Ok(match item {
                    Ok(delta) => SseEvent::default().data(
                        serde_json::json!({
                            "delta": delta.delta,
                            "finish_reason": delta.finish_reason,
                        })
                        .to_string(),
                    ),
                    Err(e) => SseEvent::default().data(
                        serde_json::json!({
                            "delta": "",
                            "finish_reason": serde_json::Value::Null,
                            "error": e.to_string(),
                        })
                        .to_string(),
                    ),
                })
            }));
        Sse::new(sse_stream)
    }

    /// Internal helper: make a streaming request to the API, parse SSE lines,
    /// and yield `StreamDelta` items (content + optional finish_reason).
    fn chat_stream_inner(
        &self,
        request: &ChatRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamDelta, CoreError>> + Send>> {
        let mut req = request.clone();
        req.stream = true;

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let req_json = match serde_json::to_string(&req) {
            Ok(s) => s,
            Err(e) => {
                return Box::pin(futures::stream::once(async move {
                    Err(CoreError::Serialization(e))
                }));
            }
        };

        stats::record_llm_call();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<StreamDelta, CoreError>>(64);
        let cancel = self.shutdown_notify.clone();

        tokio::spawn(async move {
            let result = async {
                let resp = client
                    .post(&url)
                    .header("Authorization", format!("Bearer {api_key}"))
                    .header("Content-Type", "application/json")
                    .body(req_json)
                    .send()
                    .await
                    .map_err(CoreError::Http)?;

                let status = resp.status();

                if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    let _ = tx
                        .send(Err(CoreError::RateLimited { retry_after: None }))
                        .await;
                    return Ok(());
                }

                if !status.is_success() {
                    let body = resp
                        .text()
                        .await
                        .unwrap_or_else(|e| format!("<failed to read response body: {e}>"));
                    let _ = tx
                        .send(Err(CoreError::LlmApi(format!(
                            "HTTP {}: {body}",
                            status.as_u16(),
                        ))))
                        .await;
                    return Ok(());
                }

                let mut byte_stream = resp.bytes_stream();
                let mut buffer = String::new();

                loop {
                    tokio::select! {
                        chunk_opt = byte_stream.next() => {
                            let chunk_result = match chunk_opt {
                                Some(c) => c,
                                None => break, // stream ended normally
                            };

                            let chunk = match chunk_result {
                                Ok(b) => b,
                                Err(e) => {
                                    let _ = tx.send(Err(CoreError::Http(e))).await;
                                    return Ok(());
                                }
                            };

                            let text = String::from_utf8_lossy(&chunk);
                            buffer.push_str(&text);

                            // Process complete lines from the buffer
                            while let Some(newline_pos) = buffer.find('\n') {
                                let line = buffer[..newline_pos].trim().to_string();
                                buffer = buffer[newline_pos + 1..].to_string();

                                if line.is_empty() || !line.starts_with("data: ") {
                                    continue;
                                }

                                let data = &line[6..]; // strip "data: " prefix
                                if data == "[DONE]" {
                                    return Ok(());
                                }

                                let val: serde_json::Value =
                                    match serde_json::from_str(data) {
                                        Ok(v) => v,
                                        Err(e) => {
                                            let _ = tx
                                                .send(Err(CoreError::Serialization(e)))
                                                .await;
                                            return Ok(());
                                        }
                                    };

                                let choices = val["choices"].as_array();
                                let choice = choices.and_then(|c| c.first());
                                let delta_content = choice
                                    .and_then(|c| c["delta"]["content"].as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let finish_reason = choice
                                    .and_then(|c| c["finish_reason"].as_str())
                                    .map(ToString::to_string);

                                // Also check for tool_calls in the delta
                                let tool_calls: Option<Vec<ToolCall>> = choice
                                    .and_then(|c| c["delta"]["tool_calls"].as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|tc| {
                                                serde_json::from_value(tc.clone()).ok()
                                            })
                                            .collect()
                                    });

                                let sd = StreamDelta {
                                    delta: delta_content,
                                    finish_reason,
                                    tool_calls,
                                };

                                // Send if there's any meaningful content or finish reason
                                if (!sd.delta.is_empty()
                                    || sd.finish_reason.is_some()
                                    || sd.tool_calls.is_some())
                                    && tx.send(Ok(sd)).await.is_err()
                                {
                                    // Receiver dropped — client disconnected
                                    return Ok(());
                                }
                            }
                        }
                        // Periodic heartbeat: detect receiver drop even when the
                        // LLM API is not producing output (e.g. thinking / latent).
                        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                            if tx.is_closed() {
                                debug!("LLM stream consumer disconnected, aborting stream read");
                                return Ok(());
                            }
                        }
                        // Graceful cancellation on server shutdown.
                        _ = cancel.notified() => {
                            debug!("LLM stream cancelled by shutdown signal");
                            return Ok(());
                        }
                    }
                }

                Ok(())
            };

            if let Err(e) = result.await {
                stats::record_llm_error();
                let _ = tx.send(Err(e)).await;
            } else {
                stats::record_llm_success();
            }
        });

        Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
    }
}

#[async_trait]
impl LlmClient for DeepSeekClient {
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, CoreError> {
        stats::record_llm_call();
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        debug!("LLM request to: {} (model: {})", url, request.model);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = resp.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            stats::record_llm_error();
            return Err(CoreError::RateLimited { retry_after: None });
        }

        if !status.is_success() {
            let body = resp
                .text()
                .await
                .unwrap_or_else(|e| format!("<failed to read response body: {e}>"));
            stats::record_llm_error();
            return Err(CoreError::LlmApi(format!(
                "HTTP {}: {body}",
                status.as_u16(),
            )));
        }

        let response: ChatResponse = resp.json().await?;
        stats::record_llm_success();
        info!(
            "LLM response: {} choices, model={}",
            response.choices.len(),
            response.model
        );
        Ok(response)
    }

    async fn list_models(&self) -> Result<Vec<String>, CoreError> {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        let body: Value = resp.json().await?;

        let models = body["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }

    fn chat_stream(
        &self,
        request: &ChatRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<String, CoreError>> + Send>> {
        let inner = self.chat_stream_inner(request);
        Box::pin(inner.filter_map(|item| {
            let result = match item {
                Ok(delta) => {
                    if delta.delta.is_empty() {
                        None
                    } else {
                        Some(Ok(delta.delta))
                    }
                }
                Err(e) => Some(Err(e)),
            };
            async move { result }
        }))
    }
}
