use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

use crate::channel::feishu::{CardBuilder, FeishuChannel, FeishuConfig};
use crate::channel::wechat_work::{WechatWorkChannel, WechatWorkConfig, WxMessage};
use crate::channel::webhook::{WebhookChannel, WebhookConfig};
use crate::error::ApiError;
use crate::state::AppState;

use axum::http::HeaderMap;

#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub channel_type: String,
    pub name: String,
    #[serde(default)]
    pub config: Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChannelRequest {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub config: Option<Value>,
}

/// `GET /api/channels` -- list all configured channels with their current
/// status (enabled/disabled) and configuration.
pub async fn list(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let channels = state.channel_repo.list().await?;
    Ok(Json(json!(channels)))
}

/// `POST /api/channels` -- create a new channel with the given type, name,
/// and JSON config. The channel is disabled by default; use `PUT` to enable.
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateChannelRequest>,
) -> Result<Json<Value>, ApiError> {
    let id = Uuid::new_v4().to_string();

    let channel = agent_db::models::ChannelRow {
        id,
        channel_type: body.channel_type,
        name: body.name,
        enabled: false,
        config: body.config.to_string(),
        created_at: String::new(),
        updated_at: String::new(),
    };

    state.channel_repo.create(&channel).await?;
    Ok(Json(json!(channel)))
}

/// `PUT /api/channels/{id}` -- update an existing channel's name, enabled
/// status, or config. All fields are optional.
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateChannelRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut channel = state
        .channel_repo
        .get(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Channel '{}' not found", id)))?;

    if let Some(name) = body.name {
        channel.name = name;
    }
    if let Some(enabled) = body.enabled {
        channel.enabled = enabled;
    }
    if let Some(config) = body.config {
        channel.config = config.to_string();
    }

    state.channel_repo.update(&channel).await?;
    Ok(Json(json!(channel)))
}

/// DELETE /api/channels/:id — delete a channel
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state.channel_repo.delete(&id).await?;
    Ok(Json(json!({ "deleted": true })))
}

/// POST /api/channels/:id/test — test a channel connection
pub async fn test(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    // Stub — full implementation depends on channel type
    Ok(Json(json!({
        "channel_id": id,
        "status": "ok",
        "message": "Channel test not yet implemented"
    })))
}

/// POST /api/channels/feishu/callback — Feishu event subscription callback.
///
/// Handles:
/// 1. URL verification (challenge echo)
/// 2. Event decryption (when encrypt_key is configured)
/// 3. Message parsing (text and post types)
/// 4. AI processing through SessionManager
/// 5. Response delivery via Feishu API
pub async fn feishu_callback(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    // ---- Step 1: URL verification challenge ----
    if let Some(challenge) = body.get("challenge").and_then(|c| c.as_str()) {
        tracing::info!("Feishu URL verification challenge received");
        return Ok(Json(json!({ "challenge": challenge })));
    }

    // ---- Step 2: Look up the Feishu channel configuration ----
    let channel_row = state
        .channel_repo
        .get_by_type("feishu")
        .await
        ?;

    let channel_row = match channel_row {
        Some(c) => c,
        None => {
            tracing::warn!("No Feishu channel configured in database");
            return Ok(Json(json!({ "code": 0, "msg": "no channel configured" })));
        }
    };

    if !channel_row.enabled {
        tracing::warn!("Feishu channel is disabled, ignoring event");
        return Ok(Json(json!({ "code": 0, "msg": "channel disabled" })));
    }

    let config: FeishuConfig = serde_json::from_str(&channel_row.config)
        .map_err(|e| ApiError::BadRequest(format!("Invalid Feishu config: {}", e)))?;

    let feishu_channel = FeishuChannel::new(config)
        .with_session_manager(state.session_manager.clone());

    // ---- Step 3: Parse the event (with decryption if needed) ----
    let channel_msg = match feishu_channel.parse_callback(&body).await {
        Ok(msg) => msg,
        Err(e) => {
            tracing::error!("Failed to parse Feishu event: {}", e);
            return Ok(Json(json!({ "code": 0, "msg": "parse error" })));
        }
    };

    // Don't process empty messages
    if channel_msg.content.is_empty() {
        tracing::debug!("Empty message content, skipping");
        return Ok(Json(json!({ "code": 0, "msg": "empty content" })));
    }

    // Broadcast channel message received event
    state.broadcast_event("channel_message_received", json!({
        "channel": "feishu",
        "chat_id": channel_msg.chat_id,
        "user_id": channel_msg.user_id,
        "content_preview": &channel_msg.content[..channel_msg.content.len().min(200)],
    }));

    // ---- Step 4: Rate limit check ----
    if let Err(limit_err) = feishu_channel.rate_limiter.check(&channel_msg.chat_id).await {
        tracing::warn!("Rate limit: {}", limit_err);
        // Send a warning message then bail
        let _ = feishu_channel
            .send_message(
                &channel_msg.chat_id,
                &format!("{}, please wait.", limit_err),
            )
            .await;
        return Ok(Json(json!({ "code": 0, "msg": "rate limited" })));
    }

    // ---- Step 5: Find or create a session for this chat ----
    let session_result = feishu_channel
        .find_or_create_session(
            &channel_msg.chat_id,
            &channel_msg.user_id,
            &state.session_repo,
        )
        .await;

    let session_id = match session_result {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to find/create session: {}", e);
            crate::error::record_error(&format!("Feishu session error: {}", e)).await;
            // Fallback: reply with error
            let _ = feishu_channel
                .send_message(
                    &channel_msg.chat_id,
                    "Sorry, I had trouble setting up your session. Please try again.",
                )
                .await;
            return Ok(Json(json!({ "code": 0, "msg": "session error" })));
        }
    };

    // Track per-channel message for Feishu
    state.increment_channel_msg("feishu").await;

    // ---- Step 6: Process through the AI SessionManager ----
    let ai_result = state
        .session_manager
        .process_message(&session_id, &channel_msg.content, None)
        .await;

    let response_text = match ai_result {
        Ok(text) => text,
        Err(e) => {
            tracing::error!("AI processing error for session {}: {}", session_id, e);
            crate::error::record_error(&format!("Feishu AI error (session {}): {}", session_id, e)).await;
            let _ = feishu_channel
                .send_message(
                    &channel_msg.chat_id,
                    "Sorry, I encountered an error while processing your message.",
                )
                .await;
            return Ok(Json(json!({ "code": 0, "msg": "ai error" })));
        }
    };

    // ---- Step 7: Send the response back via Feishu ----
    let has_code = response_text.contains("```");
    let is_long = response_text.len() > 500;

    if has_code || is_long {
        // Use a card for rich responses
        let card = CardBuilder::ai_response_card(&response_text);
        if let Err(e) = feishu_channel.send_card(&channel_msg.chat_id, &card).await {
            tracing::error!("Failed to send card: {}", e);
            // Fallback: send as text
            let truncated = if response_text.len() > 5000 {
                format!("{}...\n\n(Response truncated)", &response_text[..5000])
            } else {
                response_text.clone()
            };
            let _ = feishu_channel
                .send_message(&channel_msg.chat_id, &truncated)
                .await;
        }
    } else {
        // Plain text response
        if let Err(e) = feishu_channel
            .send_message(&channel_msg.chat_id, &response_text)
            .await
        {
            tracing::error!("Failed to send message: {}", e);
        }
    }

    // ---- Step 8: Acknowledge the event to Feishu ----
    Ok(Json(json!({ "code": 0, "msg": "ok" })))
}

// ============================================================
// WeChat Work Callback
// ============================================================

/// `GET /api/channels/wechat_work/callback` -- URL verification
///
/// WeChat Work sends a GET request with query parameters:
/// - `msg_signature`: SHA1 signature
/// - `timestamp`: Unix timestamp
/// - `nonce`: Random string
/// - `echostr`: Encrypted verification string to decrypt and return
pub async fn wechat_work_verify(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<String, ApiError> {
    let msg_signature = params
        .get("msg_signature")
        .map(|s| s.as_str())
        .unwrap_or("");
    let timestamp = params.get("timestamp").map(|s| s.as_str()).unwrap_or("");
    let nonce = params.get("nonce").map(|s| s.as_str()).unwrap_or("");
    let echostr = params.get("echostr").map(|s| s.as_str()).unwrap_or("");

    if echostr.is_empty() {
        return Err(ApiError::BadRequest(
            "Missing echostr parameter for URL verification".into(),
        ));
    }

    // Look up the WeChat Work channel configuration
    let channel_row = state
        .channel_repo
        .get_by_type("wechat_work")
        .await
        ?;

    let channel_row = channel_row.ok_or_else(|| {
        ApiError::NotFound(
            "No WeChat Work channel configured. Please save your WeChat Work settings first."
                .into(),
        )
    })?;

    let config: WechatWorkConfig = serde_json::from_str(&channel_row.config)
        .map_err(|e| ApiError::BadRequest(format!("Invalid WeChat Work config: {}", e)))?;

    let wx_channel =
        WechatWorkChannel::new(config).map_err(ApiError::BadRequest)?;

    let decrypted_echostr = wx_channel
        .verify_url(msg_signature, timestamp, nonce, echostr)
        .map_err(|e| {
            tracing::error!("WeChat Work URL verification failed: {}", e);
            ApiError::BadRequest(e)
        })?;

    tracing::info!("WeChat Work URL verification successful");
    Ok(decrypted_echostr)
}

/// `POST /api/channels/wechat_work/callback` -- message reception
///
/// WeChat Work sends encrypted XML POST requests for message events.
/// The body contains encrypted XML which we decrypt and parse, then
/// process through the AI SessionManager.
pub async fn wechat_work_callback(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    body: String,
) -> Result<impl IntoResponse, ApiError> {
    let msg_signature = params
        .get("msg_signature")
        .map(|s| s.as_str())
        .unwrap_or("");
    let timestamp = params.get("timestamp").map(|s| s.as_str()).unwrap_or("");
    let nonce = params.get("nonce").map(|s| s.as_str()).unwrap_or("");

    // Look up the WeChat Work channel configuration
    let channel_row = state
        .channel_repo
        .get_by_type("wechat_work")
        .await
        ?;

    let channel_row = match channel_row {
        Some(c) => c,
        None => {
            tracing::warn!("No WeChat Work channel configured in database");
            return Ok("success".to_string());
        }
    };

    if !channel_row.enabled {
        tracing::warn!("WeChat Work channel is disabled, ignoring event");
        return Ok("success".to_string());
    }

    let config: WechatWorkConfig = serde_json::from_str(&channel_row.config)
        .map_err(|e| ApiError::BadRequest(format!("Invalid WeChat Work config: {}", e)))?;

    let wx_channel = WechatWorkChannel::new(config)
        .map_err(ApiError::BadRequest)?
        .with_session_manager(state.session_manager.clone());

    // Parse the callback: signature verify + XML parse + decrypt + message parse
    let (_envelope, wx_msg) = match wx_channel.parse_callback(
        &body,
        msg_signature,
        timestamp,
        nonce,
    ) {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to parse WeChat Work callback: {}", e);
            return Ok("success".to_string());
        }
    };

    // Convert to normalized ChannelMessage
    let channel_msg = wx_channel.to_channel_message(&wx_msg);

    // Skip empty messages
    if channel_msg.content.is_empty() {
        tracing::debug!("Empty WeChat Work message content, skipping");
        return Ok("success".to_string());
    }

    // Track per-channel stats
    state.increment_channel_msg("wechat_work").await;

    // Broadcast channel message received event
    state.broadcast_event("channel_message_received", json!({
        "channel": "wechat_work",
        "chat_id": channel_msg.chat_id,
        "user_id": channel_msg.user_id,
        "content_preview": &channel_msg.content[..channel_msg.content.len().min(200)],
    }));

    // Handle subscribe events with a welcome message
    if let WxMessage::Event(_) = &wx_msg {
        // Look up from the raw ChannelMessage content
        if channel_msg.content.starts_with("User just subscribed") {
            let welcome = "Thank you for subscribing! I am your AI assistant. Send me a message to get started.";
            let _ = wx_channel
                .send_text_message(&channel_msg.user_id, welcome)
                .await;
            return Ok("success".to_string());
        }
        // For unsub / other events, acknowledge silently
        return Ok("success".to_string());
    }

    // ---- AI Processing ----

    // Find or create a session for this chat
    let session_id = match find_or_create_wx_session(
        &state,
        &channel_msg.chat_id,
        &channel_msg.user_id,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to find/create WeChat session: {}", e);
            let _ = wx_channel
                .send_text_message(
                    &channel_msg.user_id,
                    "Sorry, I had trouble setting up your session. Please try again.",
                )
                .await;
            return Ok("success".to_string());
        }
    };

    // Process through AI
    let ai_result = state
        .session_manager
        .process_message(&session_id, &channel_msg.content, None)
        .await;

    let response_text = match ai_result {
        Ok(text) => text,
        Err(e) => {
            tracing::error!(
                "AI processing error for WeChat session {}: {}",
                session_id,
                e
            );
            crate::error::record_error(&format!(
                "WeChat Work AI error (session {}): {}",
                session_id, e
            ))
            .await;
            let _ = wx_channel
                .send_text_message(
                    &channel_msg.user_id,
                    "Sorry, I encountered an error while processing your message.",
                )
                .await;
            return Ok("success".to_string());
        }
    };

    // ---- Send Response ----

    // For long responses (>500 chars), try a split approach:
    // - First message: send up to 2048 chars as text (WeChat limit for text is ~2048)
    // - For very large responses: use markdown
    let send_result = if response_text.len() > 2048 {
        // Split into chunks
        let chunks: Vec<&str> = response_text
            .as_bytes()
            .chunks(2000)
            .filter_map(|chunk| std::str::from_utf8(chunk).ok())
            .collect();

        for (i, chunk) in chunks.iter().enumerate() {
            let label = if chunks.len() > 1 {
                format!("({}/{})\n{}", i + 1, chunks.len(), chunk)
            } else {
                chunk.to_string()
            };
            if let Err(e) = wx_channel.send_text_message(&channel_msg.user_id, &label).await {
                tracing::error!("Failed to send WeChat Work response chunk {}: {}", i, e);
            }
        }
        Ok(())
    } else {
        wx_channel
            .send_text_message(&channel_msg.user_id, &response_text)
            .await
    };

    if let Err(e) = send_result {
        tracing::error!("Failed to send WeChat Work response: {}", e);
    }

    // Always return "success" to acknowledge receipt
    Ok("success".to_string())
}

/// Find or create a session for a WeChat Work user.
async fn find_or_create_wx_session(
    state: &AppState,
    chat_id: &str,
    user_id: &str,
) -> Result<String, String> {
    // Look up by channel_chat_id
    let existing = state
        .session_repo
        .list()
        .await
        .map_err(|e| format!("DB error: {}", e))?
        .into_iter()
        .find(|s| s.channel_chat_id.as_deref() == Some(chat_id));

    if let Some(session) = existing {
        state
            .session_repo
            .touch(&session.id)
            .await
            .map_err(|e| format!("DB error: {}", e))?;
        return Ok(session.id);
    }

    // Create new session
    let session_id = uuid::Uuid::new_v4().to_string();
    let session_name = format!(
        "WxWork-{}",
        &chat_id.chars().take(8).collect::<String>()
    );

    let session_row = agent_db::models::SessionRow {
        id: session_id.clone(),
        name: session_name,
        agent_id: None,
        system_prompt: None,
        model: "default".to_string(),
        temperature: 0.7,
        max_tokens: 4096,
        channel: "wechat_work".to_string(),
        channel_chat_id: Some(chat_id.to_string()),
        created_at: String::new(),
        updated_at: String::new(),
    };

    state
        .session_repo
        .create(&session_row)
        .await
        .map_err(|e| format!("Failed to create session: {}", e))?;

    tracing::info!(
        "Created new session {} for WeChat Work user {}",
        session_id,
        user_id
    );

    Ok(session_id)
}

// ============================================================
// Generic Webhook Callback
// ============================================================

/// `POST /api/channels/webhook/{path}` -- Generic webhook endpoint
///
/// Receives JSON payloads from any external service (Zapier, n8n, IFTTT,
/// custom scripts), extracts a user message from a configurable JSON path,
/// optionally verifies an HMAC-SHA256 signature, processes through the AI
/// SessionManager, and returns a formatted JSON response.
///
/// # Headers
///
/// - `X-Signature-256: <hex>` -- HMAC-SHA256 signature of the request body
///   (required only when a `secret` is configured).
///
/// # Request body
///
/// Any valid JSON object. The message is extracted using the configured
/// `json_message_path` (default: `"message"`).
///
/// # Response
///
/// Formatted according to the configured `response_template`
/// (default: `{"reply": "..."}`).
pub async fn webhook_callback(
    State(state): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, ApiError> {
    // ---- Step 1: Look up the webhook channel by webhook_url_path ----
    let all_channels = state
        .channel_repo
        .list()
        .await
        ?;

    // Find the webhook channel whose webhook_url_path matches the path segment
    let channel_row = all_channels
        .iter()
        .find(|c| {
            c.channel_type == "webhook"
                && c.enabled
                && {
                    // Parse the channel config to extract webhook_url_path
                    if let Ok(cfg) = serde_json::from_str::<WebhookConfig>(&c.config) {
                        cfg.webhook_url_path == path
                    } else {
                        false
                    }
                }
        })
        .cloned();

    let channel_row = match channel_row {
        Some(c) => c,
        None => {
            tracing::warn!(
                "No enabled webhook channel found for path '{}'",
                path
            );
            return Err(ApiError::NotFound(format!(
                "No webhook configured for path '{}'",
                path
            )));
        }
    };

    let config: WebhookConfig = serde_json::from_str(&channel_row.config)
        .map_err(|e| {
            ApiError::BadRequest(format!("Invalid webhook config: {}", e))
        })?;

    let webhook_channel = WebhookChannel::new(config.clone())
        .with_session_manager(state.session_manager.clone());

    // ---- Step 2: Verify HMAC signature (if secret is configured) ----
    let signature_header = headers
        .get("X-Signature-256")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if let Err(e) =
        webhook_channel.verify_signature(body.as_bytes(), signature_header)
    {
        tracing::warn!(
            "Webhook '{}' signature verification failed: {}",
            config.webhook_url_path,
            e
        );
        return Err(ApiError::Unauthorized(format!(
            "Signature verification failed: {}",
            e
        )));
    }

    // ---- Step 3: Parse the JSON body ----
    let payload: Value = serde_json::from_str(&body).map_err(|e| {
        ApiError::BadRequest(format!("Invalid JSON body: {}", e))
    })?;

    // ---- Step 4: Extract the user message ----
    let message_text = webhook_channel.extract_message(&payload);

    if message_text.is_empty() {
        return Err(ApiError::BadRequest(
            "Could not extract a message from request body. \
             Check your json_message_path configuration."
                .to_string(),
        ));
    }

    tracing::info!(
        "Webhook '{}': received message ({} chars)",
        config.webhook_url_path,
        message_text.len()
    );

    // Track per-channel stats
    state.increment_channel_msg("webhook").await;

    // Broadcast event
    state.broadcast_event("channel_message_received", json!({
        "channel": "webhook",
        "webhook_path": config.webhook_url_path,
        "content_preview": &message_text[..message_text.len().min(200)],
    }));

    // ---- Step 5: Find or create a session ----
    let chat_id = format!("webhook-{}", config.webhook_url_path);

    let session_id = match webhook_channel
        .find_or_create_session(&chat_id, &config.webhook_url_path, &state.session_repo)
        .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(
                "Failed to find/create webhook session: {}",
                e
            );
            crate::error::record_error(&format!(
                "Webhook session error ({}): {}",
                config.webhook_url_path, e
            ))
            .await;
            return Err(ApiError::Internal(anyhow::anyhow!(
                "Session setup failed: {}",
                e
            )));
        }
    };

    // ---- Step 6: Process through AI SessionManager ----
    let ai_result = state
        .session_manager
        .process_message(&session_id, &message_text, None)
        .await;

    let response_text = match ai_result {
        Ok(text) => text,
        Err(e) => {
            tracing::error!(
                "AI processing error for webhook session {}: {}",
                session_id,
                e
            );
            crate::error::record_error(&format!(
                "Webhook AI error (path={}, session={}): {}",
                config.webhook_url_path, session_id, e
            ))
            .await;
            return Err(ApiError::Internal(anyhow::anyhow!(
                "AI processing failed: {}",
                e
            )));
        }
    };

    // ---- Step 7: Format the response ----
    let response = webhook_channel.format_ai_response(&response_text);

    tracing::info!(
        "Webhook '{}': AI response generated ({} chars)",
        config.webhook_url_path,
        response_text.len()
    );

    Ok(Json(response))
}
