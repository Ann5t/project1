use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use async_trait::async_trait;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::{Channel, ChannelMessage, ChannelResponse, VerifyParams};

// ============================================================
// Section 1: Feishu Event Types (v2 schema)
// ============================================================

/// Top-level Feishu v2 event envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuEvent {
    pub schema: Option<String>,
    pub header: Option<FeishuEventHeader>,
    pub event: Option<Value>,
    /// Present only when the event is encrypted
    pub encrypt: Option<String>,
    /// URL verification challenge (sent during subscription setup)
    pub challenge: Option<String>,
    pub token: Option<String>,
    #[serde(rename = "type")]
    pub event_type_v1: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuEventHeader {
    pub event_id: Option<String>,
    pub event_type: Option<String>,
    pub create_time: Option<String>,
    pub token: Option<String>,
    pub app_id: Option<String>,
    pub tenant_key: Option<String>,
}

/// Parsed message event fields extracted from `event` payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMessageEvent {
    pub message_id: String,
    pub chat_id: String,
    pub msg_type: String, // "text", "post", "image", etc.
    pub content: String,  // JSON string for text/post; raw payload
    pub sender_id: FeishuSenderId,
    pub root_id: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuSenderId {
    pub union_id: Option<String>,
    pub open_id: Option<String>,
    pub user_id: Option<String>,
}

/// Configuration for a Feishu channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuConfig {
    pub app_id: String,
    pub app_secret: String,
    pub verification_token: String,
    pub encrypt_key: Option<String>,
}

// ============================================================
// Section 2: Event Parsing & Decryption
// ============================================================

/// AES-256-CBC cipher type alias
type Aes256Cbc = cbc::Decryptor<aes::Aes256>;

impl FeishuEvent {
    /// Decrypt the event body if it carries an `encrypt` field.
    ///
    /// Returns a new `FeishuEvent` with the decrypted inner event parsed
    /// into the `event` field.
    pub fn decrypt(&self, encrypt_key: &str) -> Result<Self, String> {
        let encrypted = match &self.encrypt {
            Some(e) => e,
            None => return Ok(self.clone()), // already plain
        };

        // 1. Compute SHA-256 of the encrypt key -> 32-byte AES key
        let mut hasher = Sha256::new();
        hasher.update(encrypt_key.as_bytes());
        let key: [u8; 32] = hasher
            .finalize()
            .as_slice()
            .try_into()
            .map_err(|_| "Failed to derive AES key from encrypt_key".to_string())?;

        // 2. Base64-decode the payload
        let cipher_bytes = base64::engine::general_purpose::STANDARD
            .decode(encrypted)
            .map_err(|e| format!("Base64 decode failed: {}", e))?;

        if cipher_bytes.len() < 17 {
            return Err("Encrypted payload too short".to_string());
        }

        // 3. First 16 bytes = IV, remainder = ciphertext
        let iv: &[u8; 16] = cipher_bytes[..16]
            .try_into()
            .map_err(|_| "Invalid IV length".to_string())?;
        let ciphertext = &cipher_bytes[16..];

        // 4. AES-256-CBC decrypt with PKCS7 padding
        let mut buf = ciphertext.to_vec();
        let plain_bytes = Aes256Cbc::new(&key.into(), iv.into())
            .decrypt_padded_mut::<Pkcs7>(&mut buf)
            .map_err(|e| format!("AES decryption failed: {}", e))?;

        let plain_str =
            String::from_utf8(plain_bytes.to_vec()).map_err(|e| format!("UTF-8: {}", e))?;

        debug!("Decrypted Feishu event payload");

        // 5. Parse decrypted JSON as the real event
        let mut decrypted_event: FeishuEvent =
            serde_json::from_str(&plain_str).map_err(|e| format!("Parse event: {}", e))?;

        // If the decrypt-and-parse produced `event` but the outer envelope
        // still has header-level metadata, merge it.
        decrypted_event.schema = decrypted_event.schema.or_else(|| self.schema.clone());

        Ok(decrypted_event)
    }

    /// Determine the event type from the v2 header, v1 type field, or challenge.
    pub fn event_type(&self) -> &str {
        if self.challenge.is_some() {
            return "url_verification";
        }
        if let Some(ref h) = self.header {
            if let Some(ref t) = h.event_type {
                return t.as_str();
            }
        }
        self.event_type_v1.as_deref().unwrap_or("unknown")
    }

    /// Parse the `event` field into a `FeishuMessageEvent` for message-receive events.
    pub fn parse_message_event(&self) -> Result<FeishuMessageEvent, String> {
        let event = self
            .event
            .as_ref()
            .ok_or_else(|| "Missing `event` field".to_string())?;

        let message = event
            .get("message")
            .ok_or_else(|| "Missing `event.message`".to_string())?;

        let sender = event
            .get("sender")
            .ok_or_else(|| "Missing `event.sender`".to_string())?;

        let sender_id_raw = sender
            .get("sender_id")
            .ok_or_else(|| "Missing `event.sender.sender_id`".to_string())?;

        let sender_id = FeishuSenderId {
            union_id: sender_id_raw
                .get("union_id")
                .and_then(|v| v.as_str().map(String::from)),
            open_id: sender_id_raw
                .get("open_id")
                .and_then(|v| v.as_str().map(String::from)),
            user_id: sender_id_raw
                .get("user_id")
                .and_then(|v| v.as_str().map(String::from)),
        };

        let msg_type = message
            .get("message_type")
            .or_else(|| message.get("msg_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let content = message
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("{}")
            .to_string();

        Ok(FeishuMessageEvent {
            message_id: message
                .get("message_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            chat_id: message
                .get("chat_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            msg_type,
            content,
            sender_id,
            root_id: message
                .get("root_id")
                .and_then(|v| v.as_str().map(String::from)),
            parent_id: message
                .get("parent_id")
                .and_then(|v| v.as_str().map(String::from)),
        })
    }

    /// Extract display text from a message event — works for both "text" and "post" types.
    pub fn extract_text_content(msg_event: &FeishuMessageEvent) -> String {
        match msg_event.msg_type.as_str() {
            "text" => {
                // Content is a JSON string: {"text":"hello"}
                serde_json::from_str::<Value>(&msg_event.content)
                    .ok()
                    .and_then(|c| c.get("text").and_then(|t| t.as_str().map(String::from)))
                    .unwrap_or_else(|| msg_event.content.clone())
            }
            "post" => {
                // Post content is rich text; extract plain text from each paragraph
                extract_post_text(&msg_event.content)
            }
            _ => msg_event.content.clone(),
        }
    }
}

/// Walk the Feishu "post" content JSON and collect all display text.
fn extract_post_text(content: &str) -> String {
    let root: Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return content.to_string(),
    };

    let mut texts = Vec::new();
    // content -> { "zh_cn": { "title": "...", "content": [[...], ...] } }
    // or                    { "title": "...", "content": [[...], ...] }
    if let Some(content_obj) = root.as_object() {
        for (_lang, lang_obj) in content_obj {
            if let Some(obj) = lang_obj.as_object() {
                // title
                if let Some(title) = obj.get("title").and_then(|t| t.as_str()) {
                    texts.push(title.to_string());
                }
                // content paragraphs
                if let Some(paragraphs) = obj.get("content").and_then(|c| c.as_array()) {
                    for para in paragraphs {
                        if let Some(elements) = para.as_array() {
                            for el in elements {
                                if let Some(t) = el.get("text").and_then(|v| v.as_str()) {
                                    texts.push(t.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if texts.is_empty() {
        content.to_string()
    } else {
        texts.join("\n")
    }
}

// ============================================================
// Section 3: Card Builder
// ============================================================

/// Builder for Feishu interactive card messages.
///
/// Produces a `serde_json::Value` that can be passed directly to
/// `send_card()` or embedded in `ChannelResponse::card`.
#[derive(Debug, Clone)]
pub struct CardBuilder {
    config: Value,
    header: Option<Value>,
    elements: Vec<Value>,
    i18n_elements: Option<Value>,
}

impl CardBuilder {
    /// Create a new card with wide-screen mode enabled.
    pub fn new() -> Self {
        Self {
            config: serde_json::json!({ "wide_screen_mode": true }),
            header: None,
            elements: Vec::new(),
            i18n_elements: None,
        }
    }

    /// Set the card header with a title and optional color template.
    ///
    /// Valid colors: "blue", "wathet", "turquoise", "green", "yellow",
    /// "orange", "red", "carmine", "violet", "purple", "indigo", "grey".
    pub fn header(mut self, title: &str, color: Option<&str>) -> Self {
        let mut header = serde_json::json!({
            "title": {
                "tag": "plain_text",
                "content": title
            }
        });
        if let Some(c) = color {
            header["template"] = Value::String(c.to_string());
        }
        self.header = Some(header);
        self
    }

    /// Add a plain-text block.
    pub fn add_text(mut self, text: &str) -> Self {
        self.elements.push(serde_json::json!({
            "tag": "div",
            "text": {
                "tag": "plain_text",
                "content": text
            }
        }));
        self
    }

    /// Add a Markdown-formatted block (LarkMD dialect).
    pub fn add_markdown(mut self, md: &str) -> Self {
        self.elements.push(serde_json::json!({
            "tag": "div",
            "text": {
                "tag": "lark_md",
                "content": md
            }
        }));
        self
    }

    /// Add a code block section (Markdown header + code in a div).
    pub fn add_code_block(mut self, language: &str, code: &str) -> Self {
        let header = format!("**{}**:", language);
        let body = format!("```{}\n{}\n```", language, code);
        self = self.add_markdown(&header);
        self.elements.push(serde_json::json!({
            "tag": "div",
            "text": {
                "tag": "lark_md",
                "content": body
            }
        }));
        self
    }

    /// Split text containing fenced code blocks into markdown + code-block elements.
    ///
    /// Renders regular Markdown text as divs and triple-backtick code fences as
    /// styled code blocks.
    pub fn add_markdown_with_code(self, md: &str) -> Self {
        let mut builder = self;
        let mut in_code = false;
        let mut code_lang = String::new();
        let mut code_buf = String::new();
        let mut text_buf = String::new();

        for line in md.lines() {
            if !in_code && line.trim_start().starts_with("```") {
                // Flush accumulated text
                if !text_buf.trim().is_empty() {
                    builder = builder.add_markdown(text_buf.trim());
                    text_buf.clear();
                }
                in_code = true;
                code_lang = line
                    .trim_start()
                    .trim_start_matches("```")
                    .trim()
                    .to_string();
                code_buf.clear();
            } else if in_code && line.trim() == "```" {
                // End code fence
                builder = builder.add_code_block(&code_lang, &code_buf);
                code_buf.clear();
                code_lang.clear();
                in_code = false;
            } else if in_code {
                if !code_buf.is_empty() {
                    code_buf.push('\n');
                }
                code_buf.push_str(line);
            } else {
                if !text_buf.is_empty() {
                    text_buf.push('\n');
                }
                text_buf.push_str(line);
            }
        }

        // Flush any remaining text
        if !text_buf.trim().is_empty() {
            builder = builder.add_markdown(text_buf.trim());
        }
        // Flush unclosed code block
        if in_code && !code_buf.is_empty() {
            builder = builder.add_code_block(&code_lang, &code_buf);
        }

        builder
    }

    /// Add a horizontal divider.
    pub fn add_divider(mut self) -> Self {
        self.elements.push(serde_json::json!({ "tag": "hr" }));
        self
    }

    /// Add an action row with buttons.
    ///
    /// Each button is a tuple of (text, value, button_type).
    /// Valid button types: "primary", "default", "danger".
    pub fn add_actions(mut self, buttons: Vec<(&str, &str, &str)>) -> Self {
        let actions: Vec<Value> = buttons
            .into_iter()
            .map(|(text, value, btn_type)| {
                serde_json::json!({
                    "tag": "button",
                    "text": {
                        "tag": "plain_text",
                        "content": text
                    },
                    "type": btn_type,
                    "value": { "key": value }
                })
            })
            .collect();

        self.elements.push(serde_json::json!({
            "tag": "action",
            "actions": actions
        }));
        self
    }

    /// Add a note (small grey text) element.
    pub fn add_note(mut self, text: &str) -> Self {
        self.elements.push(serde_json::json!({
            "tag": "note",
            "elements": [{
                "tag": "plain_text",
                "content": text
            }]
        }));
        self
    }

    /// Build the final card JSON.
    pub fn build(&self) -> serde_json::Value {
        let mut card = serde_json::json!({
            "config": self.config,
            "elements": self.elements,
        });

        if let Some(ref h) = self.header {
            card["header"] = h.clone();
        }
        if let Some(ref i18n) = self.i18n_elements {
            card["i18n_elements"] = i18n.clone();
        }

        card
    }

    // ---- Convenience builders for common cards ----

    /// Build a card for displaying an AI response.
    ///
    /// Automatically detects code blocks and formats them appropriately.
    pub fn ai_response_card(response_text: &str) -> serde_json::Value {
        CardBuilder::new()
            .header("AI Response", Some("blue"))
            .add_markdown_with_code(response_text)
            .add_divider()
            .add_note("Powered by AI Agent")
            .build()
    }

    /// Build a configuration prompt card with action buttons.
    pub fn config_prompt_card(
        title: &str,
        description: &str,
        buttons: Vec<(&str, &str, &str)>,
    ) -> serde_json::Value {
        let mut builder = CardBuilder::new()
            .header(title, Some("wathet"))
            .add_markdown(description);

        if !buttons.is_empty() {
            builder = builder.add_divider().add_actions(buttons);
        }

        builder.build()
    }

    /// Build an error notification card.
    pub fn error_card(title: &str, detail: &str) -> serde_json::Value {
        CardBuilder::new()
            .header(title, Some("red"))
            .add_text(detail)
            .add_note("Please try again or contact support.")
            .build()
    }

    /// Build a welcome / help card.
    pub fn help_card() -> serde_json::Value {
        CardBuilder::new()
            .header("AI Agent Help", Some("blue"))
            .add_markdown("Here are some things you can ask me:")
            .add_text("-- Ask questions about your data")
            .add_text("-- Run automated workflows")
            .add_text("-- Configure tasks and schedules")
            .add_divider()
            .add_actions(vec![
                ("Get Started", "action_get_started", "primary"),
                ("Help", "action_help", "default"),
            ])
            .add_note("Type /help anytime for this menu.")
            .build()
    }
}

impl Default for CardBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Section 4: Rate Limiter
// ============================================================

/// Shared rate limiter for Feishu channels.
///
/// Uses a sliding-window approach keyed by chat_id.
/// Thread-safe via `tokio::sync::RwLock`.
#[derive(Debug)]
pub struct ChatRateLimiter {
    windows: RwLock<HashMap<String, Vec<Instant>>>,
    max_requests: usize,
    window_duration: Duration,
}

impl ChatRateLimiter {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            windows: RwLock::new(HashMap::new()),
            max_requests,
            window_duration: Duration::from_secs(window_secs),
        }
    }

    /// Check whether `chat_id` is within the rate limit.
    ///
    /// Returns `Ok(())` if allowed, `Err(msg)` if rate-limited.
    pub async fn check(&self, chat_id: &str) -> Result<(), String> {
        let mut windows = self.windows.write().await;
        let now = Instant::now();
        let entries = windows.entry(chat_id.to_string()).or_default();

        // Remove timestamps outside the window
        let cutoff = now - self.window_duration;
        entries.retain(|t| *t > cutoff);

        if entries.len() >= self.max_requests {
            warn!(
                "Rate limit hit for chat {}: {} requests in {:?}",
                chat_id,
                entries.len(),
                self.window_duration
            );
            return Err(format!(
                "Rate limit exceeded: max {} messages per {}s",
                self.max_requests,
                self.window_duration.as_secs()
            ));
        }

        entries.push(now);
        Ok(())
    }

    /// Reset the counter for a specific chat.
    #[allow(dead_code)]
    pub async fn reset(&self, chat_id: &str) {
        self.windows.write().await.remove(chat_id);
    }
}

// ============================================================
// Section 5: FeishuChannel Implementation
// ============================================================

/// Feishu (Lark) channel implementation.
///
/// Handles event parsing, message routing through the AI SessionManager,
/// and sending responses back via the Feishu API.
pub struct FeishuChannel {
    pub app_id: String,
    pub app_secret: String,
    pub verification_token: String,
    pub encrypt_key: Option<String>,
    pub session_manager: Option<Arc<agent_core::session::manager::SessionManager>>,
    pub rate_limiter: Arc<ChatRateLimiter>,
    /// Cached tenant access token and its expiry
    token_cache: RwLock<Option<(String, Instant)>>,
    /// Shared HTTP client with timeout (30 s connect/read).
    http_client: reqwest::Client,
}

impl FeishuChannel {
    pub fn new(config: FeishuConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(5)
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to build Feishu HTTP client with custom settings: {e}. Falling back to default.");
                reqwest::Client::new()
            });
        Self {
            app_id: config.app_id,
            app_secret: config.app_secret,
            verification_token: config.verification_token,
            encrypt_key: config.encrypt_key,
            session_manager: None,
            rate_limiter: Arc::new(ChatRateLimiter::new(30, 60)), // 30 msg/min per chat
            token_cache: RwLock::new(None),
            http_client,
        }
    }

    /// Attach a SessionManager for AI-powered message handling.
    pub fn with_session_manager(
        mut self,
        sm: Arc<agent_core::session::manager::SessionManager>,
    ) -> Self {
        self.session_manager = Some(sm);
        self
    }

    // ---- Token management ----

    /// Get tenant access token for API calls (with caching).
    pub async fn get_access_token(&self) -> Result<String, String> {
        // Check cache first (5-min buffer before actual 2-hour expiry)
        {
            let cache = self.token_cache.read().await;
            if let Some((ref token, expiry)) = *cache {
                if Instant::now() < expiry {
                    return Ok(token.clone());
                }
            }
        }

        let resp = self
            .http_client
            .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
            .json(&serde_json::json!({
                "app_id": self.app_id,
                "app_secret": self.app_secret,
            }))
            .send()
            .await
            .map_err(|e| format!("Failed to get access token: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse token response: {}", e))?;

        let token = body["tenant_access_token"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| format!("No access token in response: {}", body))?;

        let expire_secs = body["expire"].as_i64().unwrap_or(7200).max(300) as u64; // at least 5 minutes

        // Cache with 5-min safety margin
        let mut cache = self.token_cache.write().await;
        *cache = Some((
            token.clone(),
            Instant::now() + Duration::from_secs(expire_secs.saturating_sub(300)),
        ));

        debug!("Feishu access token refreshed, expires in {}s", expire_secs);
        Ok(token)
    }

    // ---- Message sending ----

    /// Send a text message to a chat.
    pub async fn send_message(&self, chat_id: &str, content: &str) -> Result<(), String> {
        let token = self.get_access_token().await?;

        let body = serde_json::json!({
            "receive_id": chat_id,
            "msg_type": "text",
            "content": serde_json::json!({
                "text": content
            }).to_string()
        });

        let resp = self
            .http_client
            .post("https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id")
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to send message: {}", e))?;

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse send response: {}", e))?;

        let code = result["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            let msg = result["msg"].as_str().unwrap_or("unknown");
            error!("Feishu send_message error (code={}): {}", code, msg);
            return Err(format!("Feishu API error: {}", msg));
        }

        info!("Sent text message to feishu chat {}", chat_id);
        Ok(())
    }

    /// Send a card message to a chat.
    pub async fn send_card(&self, chat_id: &str, card: &serde_json::Value) -> Result<(), String> {
        let token = self.get_access_token().await?;

        let body = serde_json::json!({
            "receive_id": chat_id,
            "msg_type": "interactive",
            "content": card.to_string()
        });

        let resp = self
            .http_client
            .post("https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id")
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to send card: {}", e))?;

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse card response: {}", e))?;

        let code = result["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            let msg = result["msg"].as_str().unwrap_or("unknown");
            error!("Feishu send_card error (code={}): {}", code, msg);
            return Err(format!("Feishu API error: {}", msg));
        }

        info!("Sent card message to feishu chat {}", chat_id);
        Ok(())
    }

    /// Reply to a specific message in a chat (threaded reply).
    pub async fn reply_to_message(&self, message_id: &str, content: &str) -> Result<(), String> {
        let token = self.get_access_token().await?;

        let body = serde_json::json!({
            "content": serde_json::json!({
                "text": content
            }).to_string(),
            "msg_type": "text"
        });

        let resp = self
            .http_client
            .post(&format!(
                "https://open.feishu.cn/open-apis/im/v1/messages/{}/reply",
                message_id
            ))
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to reply: {}", e))?;

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse reply response: {}", e))?;

        let code = result["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            return Err(format!(
                "Feishu reply error: {}",
                result["msg"].as_str().unwrap_or("unknown")
            ));
        }

        info!("Replied to feishu message {}", message_id);
        Ok(())
    }

    // ---- Event processing ----

    /// Parse a raw Feishu callback body into a `ChannelMessage`.
    ///
    /// Handles decryption transparently when `encrypt_key` is configured.
    pub async fn parse_callback(&self, body: &Value) -> Result<ChannelMessage, String> {
        let feishu_event: FeishuEvent = serde_json::from_value(body.clone())
            .map_err(|e| format!("Failed to parse Feishu event: {}", e))?;

        // Decrypt if necessary
        let feishu_event = if let Some(ref ek) = self.encrypt_key {
            feishu_event.decrypt(ek)?
        } else {
            feishu_event
        };

        // Parse the message event
        let msg_event = feishu_event.parse_message_event()?;
        let text_content = FeishuEvent::extract_text_content(&msg_event);
        let user_id = msg_event
            .sender_id
            .open_id
            .clone()
            .or_else(|| msg_event.sender_id.user_id.clone())
            .or_else(|| msg_event.sender_id.union_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        info!(
            "Parsed feishu message: chat={}, user={}, type={}, content_len={}",
            msg_event.chat_id,
            user_id,
            msg_event.msg_type,
            text_content.len()
        );

        Ok(ChannelMessage {
            channel_type: "feishu".to_string(),
            chat_id: msg_event.chat_id,
            user_id,
            content: text_content,
            raw: body.clone(),
        })
    }

    /// Find an existing session for the chat, or create a new one.
    pub async fn find_or_create_session(
        &self,
        chat_id: &str,
        user_id: &str,
        session_repo: &agent_db::repo::SessionRepo,
    ) -> Result<String, String> {
        // Look up by channel_chat_id
        let existing = session_repo
            .list()
            .await
            .map_err(|e| format!("DB error: {}", e))?
            .into_iter()
            .find(|s| s.channel_chat_id.as_deref() == Some(chat_id));

        if let Some(session) = existing {
            debug!("Found existing session {} for chat {}", session.id, chat_id);
            session_repo
                .touch(&session.id)
                .await
                .map_err(|e| format!("DB error: {}", e))?;
            return Ok(session.id);
        }

        // Create new session
        let session_id = uuid::Uuid::new_v4().to_string();
        let session_name = format!(
            "FeishuChat-{}",
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
            channel: "feishu".to_string(),
            channel_chat_id: Some(chat_id.to_string()),
            created_at: String::new(),
            updated_at: String::new(),
        };

        session_repo
            .create(&session_row)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;

        info!(
            "Created new session {} for feishu chat {} (user {})",
            session_id, chat_id, user_id
        );

        Ok(session_id)
    }
}

// ============================================================
// Section 6: Channel trait implementation
// ============================================================

#[async_trait]
impl Channel for FeishuChannel {
    fn name(&self) -> &str {
        "Feishu"
    }

    fn channel_type(&self) -> &str {
        "feishu"
    }

    /// Handle an incoming message: parse content, route through AI, return response.
    async fn handle_message(&self, msg: ChannelMessage) -> Result<ChannelResponse, String> {
        // Parse the message content (extract text from raw event)
        let text = self.extract_content_from_raw(&msg);
        if text.is_empty() {
            return Ok(ChannelResponse {
                content: String::new(),
                card: None,
            });
        }

        // For full AI integration, use the route handler (feishu_callback)
        // which handles session creation, rate limiting, and message delivery.
        // This trait method provides a simple fallback for direct callers.
        let response_content = match &self.session_manager {
            Some(sm) => match sm.process_message("default", &text, None).await {
                Ok(resp) => resp,
                Err(e) => {
                    warn!("AI processing via handle_message failed: {}", e);
                    format!("Received: {}", text)
                }
            },
            None => {
                format!("Received: {}", text)
            }
        };

        let card = if response_content.contains("```") || response_content.len() > 500 {
            Some(CardBuilder::ai_response_card(&response_content))
        } else {
            None
        };

        Ok(ChannelResponse {
            content: response_content,
            card,
        })
    }

    async fn verify(&self, params: VerifyParams) -> Result<bool, String> {
        // Validate verification token
        if let Some(token) = params.params.get("token").and_then(|t| t.as_str()) {
            return Ok(token == self.verification_token);
        }
        Ok(false)
    }
}

// ---- Private helpers ----

impl FeishuChannel {
    /// Extract plain-text content from the raw ChannelMessage.
    fn extract_content_from_raw(&self, msg: &ChannelMessage) -> String {
        if !msg.content.is_empty() {
            return msg.content.clone();
        }

        // Fallback: try to parse the raw event
        let feishu_event: FeishuEvent = match serde_json::from_value(msg.raw.clone()) {
            Ok(e) => e,
            Err(_) => return String::new(),
        };

        // Decrypt if needed
        let feishu_event = if let Some(ref ek) = self.encrypt_key {
            match feishu_event.decrypt(ek) {
                Ok(e) => e,
                Err(err) => {
                    warn!("Event decryption failed: {}", err);
                    return String::new();
                }
            }
        } else {
            feishu_event
        };

        // Parse message event
        let msg_event = match feishu_event.parse_message_event() {
            Ok(e) => e,
            Err(err) => {
                warn!("Message parsing failed: {}", err);
                return String::new();
            }
        };

        FeishuEvent::extract_text_content(&msg_event)
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    // ── FeishuEvent parsing tests ──

    #[test]
    fn test_event_type_url_verification() {
        let event = FeishuEvent {
            schema: None,
            header: None,
            event: None,
            encrypt: None,
            challenge: Some("test-challenge-code".to_string()),
            token: Some("verification-token".to_string()),
            event_type_v1: None,
        };
        assert_eq!(event.event_type(), "url_verification");
    }

    #[test]
    fn test_event_type_from_v2_header() {
        let header = FeishuEventHeader {
            event_id: Some("evt-001".into()),
            event_type: Some("im.message.receive_v1".into()),
            create_time: Some("1234567890".into()),
            token: Some("tok".into()),
            app_id: Some("app-1".into()),
            tenant_key: Some("tenant-1".into()),
        };
        let event = FeishuEvent {
            schema: Some("2.0".into()),
            header: Some(header),
            event: None,
            encrypt: None,
            challenge: None,
            token: None,
            event_type_v1: None,
        };
        assert_eq!(event.event_type(), "im.message.receive_v1");
    }

    #[test]
    fn test_event_type_from_v1_fallback() {
        let event = FeishuEvent {
            schema: None,
            header: None,
            event: None,
            encrypt: None,
            challenge: None,
            token: None,
            event_type_v1: Some("message".to_string()),
        };
        assert_eq!(event.event_type(), "message");
    }

    #[test]
    fn test_event_type_unknown_when_nothing_set() {
        let event = FeishuEvent {
            schema: None,
            header: None,
            event: None,
            encrypt: None,
            challenge: None,
            token: None,
            event_type_v1: None,
        };
        assert_eq!(event.event_type(), "unknown");
    }

    #[test]
    fn test_parse_message_event_missing_event_field() {
        let event = FeishuEvent {
            schema: Some("2.0".into()),
            header: None,
            event: None,
            encrypt: None,
            challenge: None,
            token: None,
            event_type_v1: None,
        };
        let result = event.parse_message_event();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing `event` field"));
    }

    #[test]
    fn test_parse_message_event_missing_message_field() {
        let event = FeishuEvent {
            schema: Some("2.0".into()),
            header: None,
            event: Some(serde_json::json!({
                "sender": {
                    "sender_id": {
                        "open_id": "ou_test"
                    }
                }
            })),
            encrypt: None,
            challenge: None,
            token: None,
            event_type_v1: None,
        };
        let result = event.parse_message_event();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing `event.message`"));
    }

    #[test]
    fn test_parse_message_event_missing_sender_field() {
        let event = FeishuEvent {
            schema: Some("2.0".into()),
            header: None,
            event: Some(serde_json::json!({
                "message": {
                    "message_id": "msg-001",
                    "chat_id": "chat-001",
                    "message_type": "text",
                    "content": "{\"text\":\"hello\"}"
                }
            })),
            encrypt: None,
            challenge: None,
            token: None,
            event_type_v1: None,
        };
        let result = event.parse_message_event();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing `event.sender`"));
    }

    #[test]
    fn test_parse_message_event_missing_sender_id() {
        let event = FeishuEvent {
            schema: Some("2.0".into()),
            header: None,
            event: Some(serde_json::json!({
                "message": {
                    "message_id": "msg-001",
                    "chat_id": "chat-001",
                    "message_type": "text",
                    "content": "{\"text\":\"hello\"}"
                },
                "sender": {}
            })),
            encrypt: None,
            challenge: None,
            token: None,
            event_type_v1: None,
        };
        let result = event.parse_message_event();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Missing `event.sender.sender_id`"));
    }

    #[test]
    fn test_parse_message_event_success() {
        let event = FeishuEvent {
            schema: Some("2.0".into()),
            header: None,
            event: Some(serde_json::json!({
                "message": {
                    "message_id": "msg-001",
                    "chat_id": "chat-456",
                    "message_type": "text",
                    "content": "{\"text\":\"Hello Feishu\"}",
                    "root_id": "root-1",
                    "parent_id": "parent-1"
                },
                "sender": {
                    "sender_id": {
                        "open_id": "ou_abc123",
                        "union_id": "on_def456",
                        "user_id": "usr_ghi789"
                    }
                }
            })),
            encrypt: None,
            challenge: None,
            token: None,
            event_type_v1: None,
        };
        let result = event.parse_message_event();
        assert!(
            result.is_ok(),
            "Should parse valid message event: {:?}",
            result.err()
        );
        let msg = result.unwrap();
        assert_eq!(msg.message_id, "msg-001");
        assert_eq!(msg.chat_id, "chat-456");
        assert_eq!(msg.msg_type, "text");
        assert_eq!(msg.content, "{\"text\":\"Hello Feishu\"}");
        assert_eq!(msg.root_id.as_deref(), Some("root-1"));
        assert_eq!(msg.parent_id.as_deref(), Some("parent-1"));
        assert_eq!(msg.sender_id.open_id.as_deref(), Some("ou_abc123"));
        assert_eq!(msg.sender_id.union_id.as_deref(), Some("on_def456"));
        assert_eq!(msg.sender_id.user_id.as_deref(), Some("usr_ghi789"));
    }

    #[test]
    fn test_parse_message_event_with_msg_type_fallback() {
        let event = FeishuEvent {
            schema: Some("2.0".into()),
            header: None,
            event: Some(serde_json::json!({
                "message": {
                    "message_id": "msg-002",
                    "chat_id": "chat-002",
                    "msg_type": "post",
                    "content": "{\"title\":\"Rich post\"}"
                },
                "sender": {
                    "sender_id": {
                        "open_id": "ou_xyz"
                    }
                }
            })),
            encrypt: None,
            challenge: None,
            token: None,
            event_type_v1: None,
        };
        let msg = event.parse_message_event().unwrap();
        assert_eq!(msg.msg_type, "post");
    }

    // ── Text extraction tests ──

    #[test]
    fn test_extract_text_content_text_type() {
        let msg_event = FeishuMessageEvent {
            message_id: "m1".into(),
            chat_id: "c1".into(),
            msg_type: "text".into(),
            content: r#"{"text":"Hello World"}"#.into(),
            sender_id: FeishuSenderId {
                union_id: None,
                open_id: Some("ou".into()),
                user_id: None,
            },
            root_id: None,
            parent_id: None,
        };
        let text = FeishuEvent::extract_text_content(&msg_event);
        assert_eq!(text, "Hello World");
    }

    #[test]
    fn test_extract_text_content_text_type_without_text_field() {
        let msg_event = FeishuMessageEvent {
            message_id: "m2".into(),
            chat_id: "c2".into(),
            msg_type: "text".into(),
            content: r#"{"not_text":"something"}"#.into(),
            sender_id: FeishuSenderId {
                union_id: None,
                open_id: Some("ou2".into()),
                user_id: None,
            },
            root_id: None,
            parent_id: None,
        };
        let text = FeishuEvent::extract_text_content(&msg_event);
        // Should fall back to raw content string
        assert_eq!(text, r#"{"not_text":"something"}"#);
    }

    #[test]
    fn test_extract_text_content_unknown_type_returns_raw() {
        let msg_event = FeishuMessageEvent {
            message_id: "m3".into(),
            chat_id: "c3".into(),
            msg_type: "image".into(),
            content: r#"{"image_key":"img_123"}"#.into(),
            sender_id: FeishuSenderId {
                union_id: None,
                open_id: Some("ou3".into()),
                user_id: None,
            },
            root_id: None,
            parent_id: None,
        };
        let text = FeishuEvent::extract_text_content(&msg_event);
        assert_eq!(text, r#"{"image_key":"img_123"}"#);
    }

    #[test]
    fn test_extract_text_content_post_type() {
        let msg_event = FeishuMessageEvent {
            message_id: "m4".into(),
            chat_id: "c4".into(),
            msg_type: "post".into(),
            content: serde_json::json!({
                "zh_cn": {
                    "title": "Rich Post Title",
                    "content": [
                        [{"tag": "text", "text": "Line one"}],
                        [{"tag": "text", "text": "Line two"}, {"tag": "text", "text": "More"}]
                    ]
                }
            })
            .to_string(),
            sender_id: FeishuSenderId {
                union_id: None,
                open_id: Some("ou4".into()),
                user_id: None,
            },
            root_id: None,
            parent_id: None,
        };
        let text = FeishuEvent::extract_text_content(&msg_event);
        assert!(text.contains("Rich Post Title"));
        assert!(text.contains("Line one"));
        assert!(text.contains("Line two"));
        assert!(text.contains("More"));
    }

    // ── Card builder tests ──

    #[test]
    fn test_card_builder_basic() {
        let card = CardBuilder::new()
            .header("Test", Some("blue"))
            .add_text("Hello")
            .build();
        assert_eq!(card["header"]["title"]["content"], "Test");
        assert_eq!(card["header"]["template"], "blue");
        assert_eq!(card["config"]["wide_screen_mode"], true);
        assert_eq!(card["elements"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_card_builder_error_card() {
        let card = CardBuilder::error_card("Oops", "Something went wrong");
        assert_eq!(card["header"]["template"], "red");
        assert!(card["elements"].as_array().unwrap().len() >= 2);
    }

    #[test]
    fn test_card_builder_help_card() {
        let card = CardBuilder::help_card();
        assert_eq!(card["header"]["title"]["content"], "AI Agent Help");
    }

    #[test]
    fn test_card_builder_ai_response_card() {
        let card = CardBuilder::ai_response_card("Some response");
        assert_eq!(card["header"]["title"]["content"], "AI Response");
    }

    // ── ChatRateLimiter tests ──

    #[tokio::test]
    async fn test_chat_rate_limiter_allows_under_limit() {
        let limiter = ChatRateLimiter::new(3, 60);
        assert!(limiter.check("chat-1").await.is_ok());
        assert!(limiter.check("chat-1").await.is_ok());
        assert!(limiter.check("chat-1").await.is_ok());
    }

    #[tokio::test]
    async fn test_chat_rate_limiter_blocks_over_limit() {
        let limiter = ChatRateLimiter::new(2, 60);
        assert!(limiter.check("chat-2").await.is_ok());
        assert!(limiter.check("chat-2").await.is_ok());
        assert!(limiter.check("chat-2").await.is_err());
    }

    #[tokio::test]
    async fn test_chat_rate_limiter_separate_chats() {
        let limiter = ChatRateLimiter::new(1, 60);
        assert!(limiter.check("chat-a").await.is_ok());
        assert!(limiter.check("chat-a").await.is_err());
        // Different chat should still be allowed
        assert!(limiter.check("chat-b").await.is_ok());
    }

    #[tokio::test]
    async fn test_chat_rate_limiter_reset() {
        let limiter = ChatRateLimiter::new(1, 60);
        assert!(limiter.check("chat-r").await.is_ok());
        assert!(limiter.check("chat-r").await.is_err());
        limiter.reset("chat-r").await;
        assert!(
            limiter.check("chat-r").await.is_ok(),
            "After reset, should allow again"
        );
    }

    // ── Decryption edge cases ──

    #[test]
    fn test_decrypt_no_encrypt_field_returns_same() {
        let event = FeishuEvent {
            schema: Some("2.0".into()),
            header: None,
            event: Some(serde_json::json!({"test": true})),
            encrypt: None,
            challenge: None,
            token: None,
            event_type_v1: None,
        };
        let result = event.decrypt("any-key").unwrap();
        assert_eq!(result.event.unwrap()["test"], true);
    }

    #[test]
    fn test_decrypt_short_payload_errors() {
        let event = FeishuEvent {
            schema: None,
            header: None,
            event: None,
            encrypt: Some("YWJj".to_string()), // "abc" in base64 = 3 bytes, < 17 required bytes
            challenge: None,
            token: None,
            event_type_v1: None,
        };
        let result = event.decrypt("my-encrypt-key-32-bytes!!!!!!");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("too short") || err.contains("AES decryption failed"),
            "Expected decryption error, got: {}",
            err
        );
    }

    #[test]
    fn test_decrypt_invalid_base64_errors() {
        let event = FeishuEvent {
            schema: None,
            header: None,
            event: None,
            encrypt: Some("not-valid-base64!!@@".to_string()),
            challenge: None,
            token: None,
            event_type_v1: None,
        };
        let result = event.decrypt("my-encrypt-key-32-bytes!!!!!!");
        assert!(result.is_err());
    }
}
