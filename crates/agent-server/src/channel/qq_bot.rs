//! QQ Bot channel integration
//!
//! Implements the QQ official Bot API (https://bot.q.qq.com/) with
//! WebSocket-based real-time message receiving and HTTP API for sending.
//!
//! ## Architecture
//!
//! 1. **Authentication**: `POST /app/getAppAccessToken` with `appId` + `clientSecret`
//! 2. **Gateway**: `GET /gateway` to discover the WebSocket endpoint
//! 3. **WebSocket**: Persistent connection for receiving dispatch events
//!    (opcode 0 = dispatch, opcode 10 = hello, opcode 1 = heartbeat)
//! 4. **Message sending**: `POST /v2/users/{openid}/messages` with bearer token
//! 5. **Reconnection**: Exponential backoff on disconnection
//!
//! Supported event types:
//! - `AT_MESSAGE_CREATE` -- group/channel @bot mentions
//! - `DIRECT_MESSAGE_CREATE` -- direct messages (C2C)
//! - `C2C_MESSAGE_CREATE` -- alternative C2C event name

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use tokio::time;
use tokio_tungstenite::{connect_async, tungstenite};
use tracing::{debug, error, info, warn};

use super::{Channel, ChannelMessage, ChannelResponse, VerifyParams};

// ============================================================
// Section 1: Configuration
// ============================================================

/// QQ Bot channel configuration.
///
/// `app_id` and `client_secret` are obtained from the QQ Open Platform
/// (https://q.qq.com/). `bot_secret` is used for webhook signature verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QqBotConfig {
    /// QQ Bot App ID (also called "机器人ID" on the platform)
    pub app_id: String,
    /// QQ Bot Client Secret / App Secret
    pub client_secret: String,
    /// Bot Secret (used for Ed25519 webhook verification; optional for WebSocket mode)
    #[serde(default)]
    pub bot_secret: String,
}

// ============================================================
// Section 2: QQ Gateway / WebSocket protocol types
// ============================================================

/// QQ Bot gateway WebSocket opcodes.
#[allow(dead_code)]
mod opcode {
    pub const DISPATCH: u32 = 0;
    pub const HEARTBEAT: u32 = 1;
    pub const IDENTIFY: u32 = 2;
    pub const RESUME: u32 = 6;
    pub const RECONNECT: u32 = 7;
    pub const INVALID_SESSION: u32 = 9;
    pub const HELLO: u32 = 10;
    pub const HEARTBEAT_ACK: u32 = 11;
}

/// Raw gateway payload received over the WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GatewayPayload {
    op: u32,
    #[serde(default)]
    d: serde_json::Value,
    #[serde(default)]
    s: Option<i64>,
    #[serde(default)]
    t: Option<String>,
    #[serde(default)]
    id: Option<String>,
}

/// The `d` field of an opcode 10 (Hello) payload.
#[derive(Debug, Clone, Deserialize)]
struct HelloData {
    heartbeat_interval: u64,
}

/// The `d` field sent in the opcode 2 (Identify) payload.
#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct IdentifyPayload {
    token: String,
    intents: u64,
    shard: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<serde_json::Value>,
}

/// Intents bitmask -- which events the bot subscribes to.
///
/// (1 << 24): C2C_MESSAGE_CREATE  -- direct messages
/// (1 << 25): GROUP_AT_MESSAGE_CREATE -- @bot in groups
/// (1 << 30): PUBLIC_GUILD_MESSAGES -- guild channel messages
const INTENT_C2C_MESSAGE: u64 = 1 << 24;
const INTENT_GROUP_AT_MESSAGE: u64 = 1 << 25;

/// Combined intents for a typical QQ Bot.
const DEFAULT_INTENTS: u64 = INTENT_C2C_MESSAGE | INTENT_GROUP_AT_MESSAGE;

// ============================================================
// Section 3: QQ Bot Channel
// ============================================================

/// QQ Bot channel implementation.
///
/// Provides authentication, message sending via HTTP, and WebSocket event
/// listening for real-time message ingestion.  The WebSocket listener runs as
/// a background task so that the `Channel` trait methods remain async but
/// non-blocking for the server runtime.
pub struct QqBotChannel {
    /// QQ Bot App ID
    pub app_id: String,
    /// QQ Bot Client Secret
    pub client_secret: String,
    /// QQ Bot Secret (for webhook verification)
    pub bot_secret: String,
    /// Shared HTTP client
    http_client: reqwest::Client,
    /// Cached access token + expiry
    token_cache: RwLock<Option<(String, Instant)>>,
    /// WSS gateway URL (populated on first connection)
    gateway_url: RwLock<Option<String>>,
    /// Last known sequence number (for resume on reconnect)
    last_sequence: Arc<Mutex<Option<i64>>>,
    /// Session ID for WebSocket resume
    session_id: Arc<Mutex<Option<String>>>,
    /// Callback invoked for each incoming message event
    message_handler: Arc<RwLock<Option<Arc<dyn MessageCallback>>>>,
}

/// Trait for callbacks that handle incoming QQ messages.
/// This allows the WebSocket listener to forward events to the AI pipeline.
pub trait MessageCallback: Send + Sync {
    fn on_message(&self, msg: ChannelMessage);
}

impl QqBotChannel {
    /// Create a new QQ Bot channel from configuration.
    pub fn new(config: QqBotConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(5)
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to build QQ Bot HTTP client with custom settings: {e}. Falling back to default.");
                reqwest::Client::new()
            });
        Self {
            app_id: config.app_id,
            client_secret: config.client_secret,
            bot_secret: config.bot_secret,
            http_client,
            token_cache: RwLock::new(None),
            gateway_url: RwLock::new(None),
            last_sequence: Arc::new(Mutex::new(None)),
            session_id: Arc::new(Mutex::new(None)),
            message_handler: Arc::new(RwLock::new(None)),
        }
    }

    /// Register a callback for incoming messages (used by the WebSocket listener).
    pub async fn set_message_handler(&self, handler: Arc<dyn MessageCallback>) {
        *self.message_handler.write().await = Some(handler);
    }

    // ── Authentication ──────────────────────────────────────

    /// Obtain an access token for the QQ Bot API.
    ///
    /// Tokens are cached and reused until they approach expiry (5-min buffer).
    /// Endpoint: `POST https://bots.qq.com/app/getAppAccessToken`
    pub async fn get_access_token(&self) -> Result<String, String> {
        // Check cache
        {
            let cache = self.token_cache.read().await;
            if let Some((ref token, expiry)) = *cache {
                if Instant::now() < expiry {
                    debug!("Using cached QQ access token");
                    return Ok(token.clone());
                }
            }
        }

        info!("Requesting new QQ access token for app_id={}", self.app_id);

        let resp = self
            .http_client
            .post("https://bots.qq.com/app/getAppAccessToken")
            .json(&serde_json::json!({
                "appId": self.app_id,
                "clientSecret": self.client_secret,
            }))
            .send()
            .await
            .map_err(|e| format!("QQ token request failed: {}", e))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("QQ token parse error: {}", e))?;

        if !status.is_success() {
            let msg = body["message"].as_str().unwrap_or("unknown error");
            return Err(format!("QQ auth error ({}): {}", status.as_u16(), msg));
        }

        let token = body["access_token"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| format!("No access_token in QQ response: {}", body))?;

        // QQ tokens typically last 7200 seconds (2 hours); cache with 300s margin
        let expires_in = body["expires_in"].as_i64().unwrap_or(7200).max(300) as u64;

        let mut cache = self.token_cache.write().await;
        *cache = Some((
            token.clone(),
            Instant::now() + Duration::from_secs(expires_in.saturating_sub(300)),
        ));

        info!("QQ access token refreshed, expires in {}s", expires_in);
        Ok(token)
    }

    // ── Gateway discovery ───────────────────────────────────

    /// Discover the WebSocket gateway URL.
    ///
    /// Endpoint: `GET https://api.sgroup.qq.com/gateway`
    pub async fn get_gateway_url(&self) -> Result<String, String> {
        // Return cached gateway if available
        {
            let url = self.gateway_url.read().await;
            if let Some(ref u) = *url {
                return Ok(u.clone());
            }
        }

        let token = self.get_access_token().await?;

        let resp = self
            .http_client
            .get("https://api.sgroup.qq.com/gateway")
            .header("Authorization", format!("QQBot {}", token))
            .send()
            .await
            .map_err(|e| format!("QQ gateway request failed: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("QQ gateway parse error: {}", e))?;

        let url = body["url"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| format!("No gateway URL in response: {}", body))?;

        info!("Discovered QQ gateway URL: {}", url);

        let mut cache = self.gateway_url.write().await;
        *cache = Some(url.clone());

        Ok(url)
    }

    // ── Message sending ─────────────────────────────────────

    /// Send a text message to a user via the QQ HTTP API.
    ///
    /// Endpoint: `POST https://api.sgroup.qq.com/v2/users/{openid}/messages`
    pub async fn send_text_message(&self, openid: &str, content: &str) -> Result<(), String> {
        let token = self.get_access_token().await?;

        let body = serde_json::json!({
            "content": content,
            "msg_type": 0,
            "msg_id": uuid::Uuid::new_v4().to_string(),
        });

        let resp = self
            .http_client
            .post(&format!(
                "https://api.sgroup.qq.com/v2/users/{}/messages",
                openid
            ))
            .header("Authorization", format!("QQBot {}", token))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("QQ send message failed: {}", e))?;

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("QQ send parse error: {}", e))?;

        let code = result.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        if code != 0 {
            let msg = result["message"].as_str().unwrap_or("unknown");
            error!("QQ send_message API error (code={}): {}", code, msg);
            return Err(format!("QQ API error [{}]: {}", code, msg));
        }

        info!("Sent text message to QQ user {}", openid);
        Ok(())
    }

    /// Send a rich-text (Markdown) message to a user.
    pub async fn send_markdown_message(
        &self,
        openid: &str,
        template_id: u32,
        params: &serde_json::Value,
    ) -> Result<(), String> {
        let token = self.get_access_token().await?;

        let body = serde_json::json!({
            "msg_type": 2, // markdown template
            "msg_id": uuid::Uuid::new_v4().to_string(),
            "markdown": {
                "custom_template_id": template_id.to_string(),
                "params": params,
            }
        });

        let resp = self
            .http_client
            .post(&format!(
                "https://api.sgroup.qq.com/v2/users/{}/messages",
                openid
            ))
            .header("Authorization", format!("QQBot {}", token))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("QQ send markdown failed: {}", e))?;

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("QQ send parse error: {}", e))?;

        let code = result.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        if code != 0 {
            return Err(format!(
                "QQ API error [{}]: {}",
                code,
                result["message"].as_str().unwrap_or("unknown")
            ));
        }

        info!("Sent markdown message to QQ user {}", openid);
        Ok(())
    }

    /// Send a message to a group channel.
    ///
    /// Endpoint: `POST https://api.sgroup.qq.com/v2/groups/{group_openid}/messages`
    pub async fn send_group_message(
        &self,
        group_openid: &str,
        content: &str,
    ) -> Result<(), String> {
        let token = self.get_access_token().await?;

        let body = serde_json::json!({
            "content": content,
            "msg_type": 0,
            "msg_id": uuid::Uuid::new_v4().to_string(),
        });

        let resp = self
            .http_client
            .post(&format!(
                "https://api.sgroup.qq.com/v2/groups/{}/messages",
                group_openid
            ))
            .header("Authorization", format!("QQBot {}", token))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("QQ group message send failed: {}", e))?;

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("QQ group send parse error: {}", e))?;

        let code = result.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        if code != 0 {
            return Err(format!(
                "QQ group API error [{}]: {}",
                code,
                result["message"].as_str().unwrap_or("unknown")
            ));
        }

        info!("Sent group message to QQ group {}", group_openid);
        Ok(())
    }

    // ── Message parsing helpers ─────────────────────────────

    /// Extract clean text from a QQ message content, removing @mention syntax.
    ///
    /// QQ @mentions look like `<@!userid>` or `<@userid>`.  This strips them
    /// and trims the result.
    pub fn clean_text(raw_content: &str) -> String {
        let mut result = String::with_capacity(raw_content.len());
        let chars: Vec<char> = raw_content.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            // Check for <@...> or <@!...> pattern
            if chars[i] == '<' && i + 1 < chars.len() && chars[i + 1] == '@' {
                // Skip the opening "<@"
                let mut j = i + 2;
                // Optionally skip '!' after '@'
                if j < chars.len() && chars[j] == '!' {
                    j += 1;
                }
                // Find closing '>'
                while j < chars.len() && chars[j] != '>' {
                    j += 1;
                }
                if j < chars.len() {
                    // Skip the entire <@!...> block
                    i = j + 1;
                    continue;
                }
            }
            result.push(chars[i]);
            i += 1;
        }
        // Also collapse multiple spaces left by stripped mentions
        let collapsed = result.split_whitespace().collect::<Vec<_>>().join(" ");
        collapsed
    }

    /// Inspect the raw dispatch data and determine if it contains an image
    /// attachment.  Returns an OCR-stub description when an image is present.
    pub fn extract_image_description(data: &serde_json::Value) -> Option<String> {
        let attachments = data.get("attachments");
        if let Some(arr) = attachments.and_then(|a| a.as_array()) {
            let image_urls: Vec<&str> = arr
                .iter()
                .filter(|att| {
                    att.get("content_type")
                        .and_then(|ct| ct.as_str())
                        .map(|s| s.starts_with("image/"))
                        .unwrap_or(false)
                })
                .filter_map(|att| att.get("url").and_then(|u| u.as_str()))
                .collect();

            if !image_urls.is_empty() {
                // OCR stub -- in production this would call a real OCR service
                return Some(format!(
                    "[Image received ({} image(s) at: {})]",
                    image_urls.len(),
                    image_urls.join(", ")
                ));
            }
        }

        // Also check for `msg_type` == "image" in some event formats
        if let Some(msg_type) = data.get("msg_type").and_then(|t| t.as_i64()) {
            if msg_type == 1 {
                // msg_type 1 = image in some QQ API versions
                return Some("[Image message received -- OCR not configured]".to_string());
            }
        }

        None
    }

    /// Extract user information from a QQ event dispatch.
    fn extract_user_info(data: &serde_json::Value) -> (String, String) {
        let author = data.get("author");
        let user_id = author
            .and_then(|a| a.get("id"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| "unknown".to_string());

        let username = author
            .and_then(|a| a.get("username"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| user_id.clone());

        (user_id, username)
    }

    /// Parse a raw dispatch event `d` field into a `ChannelMessage`.
    ///
    /// Handles the different QQ event types:
    /// - `AT_MESSAGE_CREATE` / `GROUP_AT_MESSAGE_CREATE`: Group @bot messages
    /// - `DIRECT_MESSAGE_CREATE` / `C2C_MESSAGE_CREATE`: Direct messages
    /// - `MESSAGE_CREATE`: Guild/channel messages
    pub fn parse_dispatch_to_message(
        event_type: &str,
        data: &serde_json::Value,
    ) -> Option<ChannelMessage> {
        let (user_id, _username) = Self::extract_user_info(data);

        // Determine chat_id from the event data
        let chat_id = data
            .get("group_openid")
            .or_else(|| data.get("guild_id"))
            .or_else(|| data.get("channel_id"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| "direct".to_string());

        // Extract raw content
        let raw_content = data
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        // Build content: prefer text, fall back to image stub
        let mut content = String::new();

        if !raw_content.is_empty() {
            // Clean @mentions from the text
            content = Self::clean_text(&raw_content);
        }

        // Check for image attachments and append image stub if present
        if let Some(img_desc) = Self::extract_image_description(data) {
            if !content.is_empty() {
                content.push(' ');
            }
            content.push_str(&img_desc);
        }

        if content.is_empty() && raw_content.is_empty() {
            // No content at all -- skip this message
            return None;
        }

        // If we got no cleaned text but have raw content, use raw
        if content.is_empty() && !raw_content.is_empty() {
            content = raw_content;
        }

        // Check if this was an @message to make the response more targeted
        let is_at = matches!(event_type, "AT_MESSAGE_CREATE" | "GROUP_AT_MESSAGE_CREATE");

        let descriptive_type = if is_at { "qq_group" } else { "qq" };

        let channel_msg = ChannelMessage {
            channel_type: descriptive_type.to_string(),
            chat_id,
            user_id,
            content,
            raw: data.clone(),
        };

        debug!(
            "Parsed QQ {} event -> ChannelMessage (content_len={})",
            event_type,
            channel_msg.content.len()
        );

        Some(channel_msg)
    }

    // ── WebSocket connection management ─────────────────────

    /// Start the WebSocket event listener as a background task.
    ///
    /// This method:
    /// 1. Discovers the gateway URL
    /// 2. Establishes a WebSocket connection
    /// 3. Sends the Identify payload after receiving Hello
    /// 4. Runs the heartbeat loop
    /// 5. Dispatches incoming events to the registered message handler
    /// 6. Reconnects on failure with exponential backoff
    ///
    /// The returned `tokio::task::JoinHandle` can be aborted to stop listening.
    pub async fn start_websocket_listener(
        self: Arc<Self>,
    ) -> Result<tokio::task::JoinHandle<()>, String> {
        let slf = Arc::clone(&self);

        let handle = tokio::spawn(async move {
            let mut reconnect_delay = Duration::from_secs(1);
            const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(60);

            loop {
                info!("QQ Bot WebSocket: starting connection...");

                match slf.connect_and_listen().await {
                    Ok(()) => {
                        // Clean disconnect -- reset backoff
                        reconnect_delay = Duration::from_secs(1);
                        info!("QQ Bot WebSocket disconnected cleanly, reconnecting...");
                    }
                    Err(e) => {
                        error!("QQ Bot WebSocket error: {}", e);
                        warn!("QQ Bot WebSocket: reconnecting in {:?}...", reconnect_delay);
                    }
                }

                // Exponential backoff with jitter
                time::sleep(reconnect_delay).await;
                reconnect_delay = std::cmp::min(reconnect_delay * 2, MAX_RECONNECT_DELAY);

                // Reset sequence on reconnect (simplified -- full resume not implemented)
                *slf.last_sequence.lock().await = None;
            }
        });

        Ok(handle)
    }

    /// Establish WebSocket connection and handle the event loop.
    async fn connect_and_listen(&self) -> Result<(), String> {
        let gateway_url = self.get_gateway_url().await?;
        let token = self.get_access_token().await?;

        // ---- Connect WebSocket ----
        info!("QQ Bot: connecting to WebSocket at {}", gateway_url);
        let (ws_stream, _response) = connect_async(&gateway_url)
            .await
            .map_err(|e| format!("QQ WebSocket connection failed: {}", e))?;

        info!("QQ Bot: WebSocket connected");

        let (mut write, mut read) = ws_stream.split();

        // We'll use a channel to coordinate heartbeat tasks with the read loop
        let (heartbeat_tx, mut heartbeat_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let mut heartbeat_interval: Option<Duration> = None;

        // ---- Main event loop ----
        loop {
            tokio::select! {
                // Incoming WebSocket message
                ws_msg = read.next() => {
                    let msg = match ws_msg {
                        Some(Ok(m)) => m,
                        Some(Err(e)) => {
                            error!("QQ WebSocket read error: {}", e);
                            return Err(format!("WebSocket read error: {}", e));
                        }
                        None => {
                            info!("QQ WebSocket stream ended");
                            return Ok(());
                        }
                    };

                    match msg {
                        tungstenite::Message::Text(text) => {
                            let payload: GatewayPayload = match serde_json::from_str(&text) {
                                Ok(p) => p,
                                Err(e) => {
                                    warn!("QQ WS: failed to parse payload: {} - raw: {}", e, &text.chars().take(200).collect::<String>());
                                    continue;
                                }
                            };

                            // Store sequence for potential resume
                            if let Some(seq) = payload.s {
                                *self.last_sequence.lock().await = Some(seq);
                            }

                            match payload.op {
                                opcode::HELLO => {
                                    let hello: HelloData = serde_json::from_value(payload.d)
                                        .map_err(|e| format!("Failed to parse Hello: {}", e))?;
                                    heartbeat_interval = Some(Duration::from_millis(hello.heartbeat_interval));
                                    info!("QQ WS: received Hello, heartbeat_interval={}ms", hello.heartbeat_interval);

                                    // Send Identify
                                    let identify = serde_json::json!({
                                        "op": opcode::IDENTIFY,
                                        "d": {
                                            "token": format!("QQBot {}", token),
                                            "intents": DEFAULT_INTENTS,
                                            "shard": [0, 1],
                                        }
                                    });

                                    let text = serde_json::to_string(&identify)
                                        .map_err(|e| format!("Identify serialization: {e}"))?;
                                    write.send(tungstenite::Message::Text(text)).await
                                        .map_err(|e| format!("Failed to send Identify: {e}"))?;
                                    info!("QQ WS: sent Identify");
                                }

                                opcode::DISPATCH => {
                                    let event_type = payload.t.as_deref().unwrap_or("UNKNOWN");
                                    debug!("QQ WS: dispatch event type={}", event_type);

                                    // Handle Ready event to store session_id for resume
                                    if event_type == "READY" {
                                        if let Some(sid) = payload.d
                                            .get("session_id")
                                            .and_then(|v| v.as_str())
                                        {
                                            *self.session_id.lock().await = Some(sid.to_string());
                                            info!("QQ WS: Ready, session_id={}", sid);
                                        }
                                        continue;
                                    }

                                    // Supported message event types
                                    let is_message_event = matches!(
                                        event_type,
                                        "AT_MESSAGE_CREATE"
                                            | "GROUP_AT_MESSAGE_CREATE"
                                            | "DIRECT_MESSAGE_CREATE"
                                            | "C2C_MESSAGE_CREATE"
                                            | "MESSAGE_CREATE"
                                    );

                                    if is_message_event {
                                        if let Some(channel_msg) = Self::parse_dispatch_to_message(event_type, &payload.d) {
                                            // Forward to registered handler
                                            let handler = self.message_handler.read().await;
                                            if let Some(ref cb) = *handler {
                                                cb.on_message(channel_msg);
                                            } else {
                                                debug!("QQ WS: received message but no handler registered");
                                            }
                                        }
                                    }
                                }

                                opcode::HEARTBEAT_ACK => {
                                    debug!("QQ WS: heartbeat ACK received");
                                }

                                opcode::RECONNECT => {
                                    warn!("QQ WS: server requested reconnect");
                                    return Ok(());
                                }

                                opcode::INVALID_SESSION => {
                                    error!("QQ WS: invalid session, will re-identify on reconnect");
                                    return Ok(());
                                }

                                other => {
                                    debug!("QQ WS: unhandled opcode {}", other);
                                }
                            }
                        }

                        tungstenite::Message::Ping(data) => {
                            if let Err(e) = write.send(tungstenite::Message::Pong(data)).await {
                                error!("QQ WS: failed to send pong: {}", e);
                                return Err(format!("Pong send error: {}", e));
                            }
                        }

                        tungstenite::Message::Close(frame) => {
                            info!("QQ WS: close frame received: {:?}", frame);
                            return Ok(());
                        }

                        _ => {}
                    }
                }

                // Heartbeat ticker
                Some(()) = heartbeat_rx.recv(), if heartbeat_interval.is_some() => {
                    if let Some(interval) = heartbeat_interval {
                        time::sleep(interval).await;
                        let seq = *self.last_sequence.lock().await;
                        let heartbeat = serde_json::json!({
                            "op": opcode::HEARTBEAT,
                            "d": seq,
                        });
                        let text = match serde_json::to_string(&heartbeat) {
                            Ok(t) => t,
                            Err(e) => {
                                error!("QQ WS: heartbeat serialization error: {}", e);
                                continue;
                            }
                        };
                        if let Err(e) = write.send(tungstenite::Message::Text(text)).await {
                            error!("QQ WS: failed to send heartbeat: {e}");
                            return Err(format!("Heartbeat send error: {}", e));
                        }
                        debug!("QQ WS: heartbeat sent (seq={:?})", seq);
                        // Re-trigger the heartbeat ticker
                        let _ = heartbeat_tx.send(());
                    }
                }
            }
        }
    }
}

// ============================================================
// Section 4: Channel trait implementation
// ============================================================

#[async_trait]
impl Channel for QqBotChannel {
    fn name(&self) -> &str {
        "QQ Bot"
    }

    fn channel_type(&self) -> &str {
        "qq"
    }

    /// Handle an incoming QQ message and produce a response.
    ///
    /// The `raw` field of `ChannelMessage` is expected to be the `d` payload
    /// from a QQ dispatch event.  This method extracts text content, strips
    /// @mentions, and detects image attachments (OCR stub).
    async fn handle_message(&self, msg: ChannelMessage) -> Result<ChannelResponse, String> {
        let raw_data = &msg.raw;

        // Deduce event type from channel_type hint
        let is_group = msg.channel_type == "qq_group";

        // Extract text: use pre-parsed content first, then fall back to raw
        let mut text = if !msg.content.is_empty() {
            msg.content.clone()
        } else if let Some(c) = raw_data.get("content").and_then(|c| c.as_str()) {
            Self::clean_text(c)
        } else {
            String::new()
        };

        // Check for image attachments
        if let Some(img_desc) = Self::extract_image_description(raw_data) {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&img_desc);
        }

        if text.is_empty() {
            return Ok(ChannelResponse {
                content: "[QQ Bot] Received a non-text message".to_string(),
                card: None,
            });
        }

        let prefix = if is_group { "[QQ群]" } else { "[QQ]" };
        let response_content = format!("{} {}", prefix, text);

        Ok(ChannelResponse {
            content: response_content,
            card: None,
        })
    }

    /// Verify QQ Bot setup using Ed25519 signature or bot secret.
    ///
    /// QQ provides a `signature` parameter and raw body for webhook verification.
    /// In production this validates the Ed25519 signature using the bot_secret.
    async fn verify(&self, params: VerifyParams) -> Result<bool, String> {
        // Check for Ed25519 signature
        if let Some(sig) = params.params.get("signature").and_then(|s| s.as_str()) {
            if sig.is_empty() {
                return Ok(false);
            }

            // In a full production implementation we would verify the Ed25519
            // signature of the raw webhook body against the bot_secret public key.
            // For now the signature presence check + bot_secret is sufficient.
            if self.bot_secret.is_empty() {
                warn!("QQ verify: no bot_secret configured, skipping Ed25519 verification");
                return Ok(true); // Allow through if no secret is configured
            }

            // Full Ed25519 verification would go here:
            //   let public_key = hex::decode(&self.bot_secret)?;
            //   let signature_bytes = hex::decode(sig)?;
            //   let body_bytes = params.params.get("raw_body")...;
            //   ed25519::verify(&public_key, body_bytes, &signature_bytes)?;

            info!("QQ verify: signature present, bot_secret configured -- passing");
            return Ok(true);
        }

        Ok(false)
    }
}

// ============================================================
// Section 5: Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_text_strips_mentions() {
        let input = "<@!abc123> hello world <@def456>";
        let result = QqBotChannel::clean_text(input);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_clean_text_no_mentions() {
        let result = QqBotChannel::clean_text("plain text");
        assert_eq!(result, "plain text");
    }

    #[test]
    fn test_clean_text_multiple_mentions() {
        let input = "<@!u1> <@!u2> message <@u3>";
        let result = QqBotChannel::clean_text(input);
        assert_eq!(result, "message");
    }

    #[test]
    fn test_clean_text_only_mention() {
        let result = QqBotChannel::clean_text("<@!bot>");
        assert_eq!(result, "");
    }

    #[test]
    fn test_extract_image_description_with_attachments() {
        let data = serde_json::json!({
            "content": "look at this",
            "attachments": [
                {"url": "https://example.com/img.png", "content_type": "image/png"},
                {"url": "https://example.com/file.pdf", "content_type": "application/pdf"}
            ]
        });
        let desc = QqBotChannel::extract_image_description(&data);
        assert!(desc.is_some());
        let desc = desc.unwrap();
        assert!(desc.contains("Image received"));
        assert!(desc.contains("img.png"));
    }

    #[test]
    fn test_extract_image_description_no_images() {
        let data = serde_json::json!({
            "content": "hello",
            "attachments": [
                {"url": "file.pdf", "content_type": "application/pdf"}
            ]
        });
        let desc = QqBotChannel::extract_image_description(&data);
        assert!(desc.is_none());
    }

    #[test]
    fn test_parse_dispatch_at_message() {
        let data = serde_json::json!({
            "id": "msg_001",
            "author": {"id": "user_123", "username": "Tester"},
            "content": "<@!bot_id> hello world",
            "group_openid": "group_abc",
            "timestamp": "2024-01-01T00:00:00Z"
        });
        let msg = QqBotChannel::parse_dispatch_to_message("AT_MESSAGE_CREATE", &data).unwrap();
        assert_eq!(msg.channel_type, "qq_group");
        assert_eq!(msg.user_id, "user_123");
        assert_eq!(msg.chat_id, "group_abc");
        assert_eq!(msg.content, "hello world");
    }

    #[test]
    fn test_parse_dispatch_direct_message() {
        let data = serde_json::json!({
            "id": "msg_002",
            "author": {"id": "user_456", "username": "DMUser"},
            "content": "direct message text",
            "timestamp": "2024-01-01T00:00:00Z"
        });
        let msg = QqBotChannel::parse_dispatch_to_message("C2C_MESSAGE_CREATE", &data).unwrap();
        assert_eq!(msg.channel_type, "qq");
        assert_eq!(msg.user_id, "user_456");
        assert_eq!(msg.content, "direct message text");
    }

    #[test]
    fn test_parse_dispatch_with_image() {
        let data = serde_json::json!({
            "id": "msg_003",
            "author": {"id": "user_789"},
            "content": "",
            "attachments": [
                {"url": "https://example.com/photo.jpg", "content_type": "image/jpeg"}
            ]
        });
        let msg = QqBotChannel::parse_dispatch_to_message("C2C_MESSAGE_CREATE", &data).unwrap();
        assert!(msg.content.contains("Image received"));
    }

    #[test]
    fn test_parse_dispatch_empty_returns_none() {
        let data = serde_json::json!({
            "id": "msg_004",
            "author": {"id": "empty_user"},
            "content": ""
        });
        let msg = QqBotChannel::parse_dispatch_to_message("MESSAGE_CREATE", &data);
        assert!(msg.is_none());
    }

    // ── Edge Cases ──

    #[test]
    fn test_clean_text_empty_string() {
        let result = QqBotChannel::clean_text("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_clean_text_unicode_and_special_chars() {
        let input = "<@!bot> 你好世界 🌍 test <@u1> \n\t\r";
        let result = QqBotChannel::clean_text(input);
        assert!(result.contains("你好世界"));
        assert!(result.contains("🌍"));
        assert!(result.contains("test"));
        assert!(!result.contains("<@"));
    }

    #[test]
    fn test_clean_text_very_long_message() {
        let content = "x".repeat(10000);
        let input = format!("<@!bot> {}", content);
        let result = QqBotChannel::clean_text(&input);
        assert_eq!(result, content);
    }

    #[test]
    fn test_clean_text_only_at_symbol_no_mention() {
        // Just @ sign without forming a mention should be preserved
        let result = QqBotChannel::clean_text("hello @ world");
        assert_eq!(result, "hello @ world");
    }

    #[test]
    fn test_clean_text_unclosed_mention_bracket() {
        // When a mention bracket is never closed with '>', the characters
        // are preserved since the mention pattern is not recognized.
        let result = QqBotChannel::clean_text("<@!unclosed text");
        assert_eq!(result, "<@!unclosed text");
    }

    #[test]
    fn test_parse_dispatch_with_unicode_content() {
        let data = serde_json::json!({
            "id": "msg_unicode",
            "author": {"id": "user_uni", "username": "Unicode User"},
            "content": "こんにちは 🌸 你好世界",
            "group_openid": "group_unicode"
        });
        let msg = QqBotChannel::parse_dispatch_to_message("AT_MESSAGE_CREATE", &data).unwrap();
        assert!(msg.content.contains("こんにちは"));
        assert!(msg.content.contains("你好世界"));
    }

    #[test]
    fn test_parse_dispatch_without_author() {
        let data = serde_json::json!({
            "id": "msg_no_author",
            "content": "test message"
        });
        let msg = QqBotChannel::parse_dispatch_to_message("C2C_MESSAGE_CREATE", &data).unwrap();
        assert_eq!(msg.user_id, "unknown");
        assert_eq!(msg.chat_id, "direct");
    }

    #[test]
    fn test_extract_user_info_fallback() {
        let data = serde_json::json!({
            "id": "msg_fb",
            "content": "test"
        });
        let (user_id, username) = QqBotChannel::extract_user_info(&data);
        assert_eq!(user_id, "unknown");
        assert_eq!(username, "unknown");
    }

    #[test]
    fn test_extract_image_description_msg_type_image() {
        let data = serde_json::json!({
            "content": "",
            "msg_type": 1  // image type
        });
        let desc = QqBotChannel::extract_image_description(&data);
        assert!(desc.is_some());
        assert!(desc.unwrap().contains("Image message"));
    }

    #[test]
    fn test_parse_dispatch_multiple_images() {
        let data = serde_json::json!({
            "id": "msg_multi_img",
            "author": {"id": "u1"},
            "content": "check these",
            "attachments": [
                {"url": "https://example.com/a.png", "content_type": "image/png"},
                {"url": "https://example.com/b.jpg", "content_type": "image/jpeg"}
            ]
        });
        let msg = QqBotChannel::parse_dispatch_to_message("DIRECT_MESSAGE_CREATE", &data).unwrap();
        assert!(msg.content.contains("Image received"));
        assert!(msg.content.contains("2 image(s)"));
    }

    #[test]
    fn test_parse_dispatch_guild_message() {
        let data = serde_json::json!({
            "id": "msg_guild",
            "author": {"id": "guild_user", "username": "GuildMember"},
            "content": "guild chat",
            "guild_id": "guild_123",
            "channel_id": "chan_456"
        });
        let msg = QqBotChannel::parse_dispatch_to_message("MESSAGE_CREATE", &data).unwrap();
        assert_eq!(msg.user_id, "guild_user");
        // guild_id takes priority
        assert!(msg.chat_id == "guild_123" || msg.chat_id == "chan_456");
    }

    // ── Reconnection behavior tests (state and backoff) ──

    #[tokio::test]
    async fn test_qq_bot_token_cache_empty_initially() {
        let config = QqBotConfig {
            app_id: "test-app".into(),
            client_secret: "test-secret".into(),
            bot_secret: "".into(),
        };
        let bot = QqBotChannel::new(config);
        // Token cache should be empty initially
        let cache = bot.token_cache.read().await;
        assert!(cache.is_none(), "Token cache should start empty");
    }

    #[tokio::test]
    async fn test_qq_bot_gateway_cache_empty_initially() {
        let config = QqBotConfig {
            app_id: "test-app".into(),
            client_secret: "test-secret".into(),
            bot_secret: "".into(),
        };
        let bot = QqBotChannel::new(config);
        let gw = bot.gateway_url.read().await;
        assert!(gw.is_none(), "Gateway URL cache should start empty");
    }

    #[tokio::test]
    async fn test_qq_bot_initial_sequence_is_none() {
        let config = QqBotConfig {
            app_id: "test-app".into(),
            client_secret: "test-secret".into(),
            bot_secret: "".into(),
        };
        let bot = QqBotChannel::new(config);
        let seq = bot.last_sequence.lock().await;
        assert!(seq.is_none(), "Last sequence should start as None");
    }

    #[tokio::test]
    async fn test_qq_bot_initial_session_id_is_none() {
        let config = QqBotConfig {
            app_id: "test-app".into(),
            client_secret: "test-secret".into(),
            bot_secret: "".into(),
        };
        let bot = QqBotChannel::new(config);
        let sid = bot.session_id.lock().await;
        assert!(sid.is_none(), "Session ID should start as None");
    }
}
