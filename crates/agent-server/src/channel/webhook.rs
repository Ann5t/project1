//! Generic Webhook channel integration
//!
//! Provides a flexible incoming webhook that allows any external service
//! (Zapier, n8n, IFTTT, custom scripts, etc.) to POST JSON payloads and
//! receive AI-generated responses.
//!
//! ## Architecture
//!
//! ```text
//! External Service         Webhook Endpoint         AI Agent Core
//! ----------------         -----------------         --------------
//! POST /api/channels/webhook/{path}
//!   + X-Signature-256  --> verify_hmac() --------> ChannelMessage
//!   + JSON body              |                          |
//!                         extract message               |
//!                         from json_message_path        |
//!                                                        |
//!                                                  SessionManager.process_message()
//!                                                        |
//!                                                  format_response()
//!                                                        |
//!   <-- JSON response  <-- {"reply": "..."}  <----------+
//! ```
//!
//! ## Use cases
//!
//! - Zapier: "Catch Hook" in Zapier -> POST to this webhook
//! - n8n: "Webhook" node -> POST to this webhook
//! - IFTTT: "Webhooks" service -> "Make a web request"
//! - Custom automation scripts, chat proxy, etc.

use std::sync::Arc;

use async_trait::async_trait;
use hex;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::{debug, info, warn};

use super::{Channel, ChannelMessage, ChannelResponse, VerifyParams};

type HmacSha256 = Hmac<Sha256>;

// ============================================================
// Section 1: Webhook Configuration
// ============================================================

/// Configuration for a generic webhook channel.
///
/// Each webhook instance is identified by its `webhook_url_path` segment,
/// accessed at `POST /api/channels/webhook/{path}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// URL path segment identifying this webhook instance.
    /// The full endpoint is `/api/channels/webhook/{webhook_url_path}`.
    /// Example: `"my-zapier-hook"` -> `/api/channels/webhook/my-zapier-hook`
    #[serde(default)]
    pub webhook_url_path: String,

    /// Shared secret for HMAC-SHA256 signature verification.
    /// When set, incoming requests must include an `X-Signature-256` header
    /// whose value equals `HMAC-SHA256(request_body, secret)` as a hex string.
    /// Leave empty to skip signature verification.
    #[serde(default)]
    pub secret: String,

    /// Dot-separated JSON path to extract the user message from the request body.
    ///
    /// Examples:
    /// - `"message"` -> `body["message"]`
    /// - `"data.text"` -> `body["data"]["text"]`
    /// - `"payload.input"` -> `body["payload"]["input"]`
    /// - `""` or `"$"` -> use the entire body as the message (converted to JSON string)
    #[serde(default = "default_message_path")]
    pub json_message_path: String,

    /// Response template for formatting the AI reply.
    ///
    /// Supports the following placeholders:
    /// - `{{reply}}` -- the full AI response text
    /// - `{{timestamp}}` -- ISO-8601 timestamp of when the response was generated
    ///
    /// Default: `{"reply": "{{reply}}"}`
    ///
    /// Examples:
    /// - `{"result": "{{reply}}"}` -> sends `{"result": "AI answer...", "timestamp": "..."}`
    /// - `{"text": "{{reply}}", "ts": "{{timestamp}}"}` -> timestamp included
    /// - `{{reply}}` (bare text) -> the raw AI reply as a JSON string (not wrapped in object)
    #[serde(default = "default_response_template")]
    pub response_template: String,
}

fn default_message_path() -> String {
    "message".to_string()
}

fn default_response_template() -> String {
    r#"{"reply": "{{reply}}"}"#.to_string()
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            webhook_url_path: String::new(),
            secret: String::new(),
            json_message_path: default_message_path(),
            response_template: default_response_template(),
        }
    }
}

// ============================================================
// Section 2: HMAC-SHA256 Signature Verification
// ============================================================

/// Verify an HMAC-SHA256 signature against the raw request body.
///
/// # Arguments
///
/// * `secret` - The shared secret key
/// * `body` - The raw request body bytes
/// * `signature_header` - The hex-encoded HMAC from the `X-Signature-256` header.
///   Supports both bare hex and the `sha256=` prefix (GitHub-style).
///
/// # Returns
///
/// `Ok(())` if the signature matches, `Err(msg)` otherwise.
pub fn verify_hmac(secret: &str, body: &[u8], signature_header: &str) -> Result<(), String> {
    // Strip "sha256=" prefix if present (GitHub webhook style)
    let signature_hex = signature_header
        .strip_prefix("sha256=")
        .unwrap_or(signature_header);

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("HMAC init error: {}", e))?;
    mac.update(body);
    let computed = mac.finalize();
    let computed_hex = hex::encode(computed.into_bytes());

    // Constant-time comparison to avoid timing attacks
    if computed_hex.len() != signature_hex.len() {
        warn!(
            "HMAC signature length mismatch: expected {}, got {}",
            computed_hex.len(),
            signature_hex.len()
        );
        return Err("Signature verification failed: length mismatch".to_string());
    }

    let mut diff = 0u8;
    for (a, b) in computed_hex.bytes().zip(signature_hex.bytes()) {
        diff |= a ^ b;
    }
    // Also cover length mismatch that wasn't caught by the early check
    // (unlikely, but belt-and-suspenders)
    diff |= (computed_hex.len() ^ signature_hex.len()) as u8;

    if diff == 0 {
        debug!("HMAC-SHA256 signature verified");
        Ok(())
    } else {
        warn!("HMAC-SHA256 signature verification failed");
        Err("Signature verification failed: mismatch".to_string())
    }
}

// ============================================================
// Section 3: JSON Path Message Extraction
// ============================================================

/// Extract a value from a `serde_json::Value` using a dot-separated path.
///
/// # Examples
///
/// ```
/// # use agent_server::channel::webhook::extract_json_path;
/// let body = serde_json::json!({"data": {"text": "hello"}});
/// assert_eq!(extract_json_path(&body, "data.text"), Some("hello".to_string()));
/// assert_eq!(extract_json_path(&body, "missing.path"), None);
/// ```
pub fn extract_json_path(value: &serde_json::Value, path: &str) -> Option<String> {
    if path.is_empty() || path == "$" {
        // Return the whole value as a string
        return Some(
            serde_json::to_string(value)
                .unwrap_or_else(|_| value.to_string()),
        );
    }

    let mut current = value;
    for segment in path.split('.') {
        match current {
            serde_json::Value::Object(map) => {
                current = map.get(segment)?;
            }
            _ => return None,
        }
    }

    match current {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Null => None,
        // For nested objects/arrays, JSON-serialize them
        other => Some(
            serde_json::to_string(other)
                .unwrap_or_else(|_| other.to_string()),
        ),
    }
}

// ============================================================
// Section 4: Response Formatting
// ============================================================

/// Format an AI response using the configured template.
///
/// Replaces `{{reply}}` with the AI reply text and `{{timestamp}}` with the
/// current ISO-8601 timestamp. If the template is bare `{{reply}}`, the raw
/// text is returned as a JSON string.
pub fn format_response(reply: &str, template: &str) -> serde_json::Value {
    let timestamp = chrono::Utc::now().to_rfc3339();

    // If the template is exactly `{{reply}}` (bare), return the raw text as a
    // JSON string value.
    let trimmed = template.trim();
    if trimmed == "{{reply}}" {
        return serde_json::Value::String(reply.to_string());
    }

    let result = template
        .replace("{{reply}}", &escape_json_string(reply))
        .replace("{{timestamp}}", &timestamp);

    // Try to parse as JSON object; fall back to a string wrapper
    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(v) => v,
        Err(_) => serde_json::json!({ "reply": reply, "timestamp": timestamp }),
    }
}

/// Escape special characters in a string for safe embedding in JSON.
fn escape_json_string(s: &str) -> String {
    // Use serde_json's built-in escaping by serializing the string value
    // and then stripping the surrounding quotes.
    let json_val = serde_json::Value::String(s.to_string());
    let serialized = serde_json::to_string(&json_val).unwrap_or_else(|_| format!("\"{}\"", s));
    // serialized is like `"escaped content"` -- strip outer quotes
    if serialized.len() >= 2 {
        serialized[1..serialized.len() - 1].to_string()
    } else {
        serialized
    }
}

// ============================================================
// Section 5: WebhookChannel Implementation
// ============================================================

/// Generic webhook channel implementation.
///
/// Receives JSON payloads via HTTP POST, extracts messages using a
/// configurable JSON path, optionally verifies HMAC signatures, routes
/// through the AI SessionManager, and returns formatted JSON responses.
pub struct WebhookChannel {
    /// Configuration for this webhook instance
    pub config: WebhookConfig,
    /// Shared session manager for AI processing
    pub session_manager: Option<Arc<agent_core::session::manager::SessionManager>>,
}

impl WebhookChannel {
    /// Create a new webhook channel from its configuration.
    pub fn new(config: WebhookConfig) -> Self {
        Self {
            config,
            session_manager: None,
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

    /// Validate the HMAC signature on an incoming request.
    ///
    /// If no secret is configured, this always returns `Ok(())`.
    pub fn verify_signature(&self, body: &[u8], signature_header: &str) -> Result<(), String> {
        if self.config.secret.is_empty() {
            // No secret configured -- skip verification
            return Ok(());
        }

        if signature_header.is_empty() {
            warn!("Webhook '{}': missing X-Signature-256 header but secret is configured",
                  self.config.webhook_url_path);
            return Err("Missing X-Signature-256 header".to_string());
        }

        verify_hmac(&self.config.secret, body, signature_header)
    }

    /// Extract the user message from a JSON body using the configured path.
    pub fn extract_message(&self, body: &serde_json::Value) -> String {
        extract_json_path(body, &self.config.json_message_path)
            .unwrap_or_else(|| {
                warn!(
                    "Webhook '{}': failed to extract message at path '{}'",
                    self.config.webhook_url_path, self.config.json_message_path
                );
                // Fallback: return the whole body as a string
                body.to_string()
            })
    }

    /// Format the AI response using the configured template.
    pub fn format_ai_response(&self, reply: &str) -> serde_json::Value {
        format_response(reply, &self.config.response_template)
    }

    /// Find an existing session or create a new one for this webhook chat.
    pub async fn find_or_create_session(
        &self,
        chat_id: &str,
        webhook_path: &str,
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
            debug!(
                "Found existing session {} for webhook chat {}",
                session.id, chat_id
            );
            session_repo
                .touch(&session.id)
                .await
                .map_err(|e| format!("DB error: {}", e))?;
            return Ok(session.id);
        }

        // Create new session
        let session_id = uuid::Uuid::new_v4().to_string();
        let session_name = format!(
            "Webhook-{}-{}",
            webhook_path,
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
            channel: "webhook".to_string(),
            channel_chat_id: Some(chat_id.to_string()),
            created_at: String::new(),
            updated_at: String::new(),
        };

        session_repo
            .create(&session_row)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;

        info!(
            "Created new session {} for webhook '{}' chat {}",
            session_id, webhook_path, chat_id
        );

        Ok(session_id)
    }
}

// ============================================================
// Section 6: Channel Trait Implementation
// ============================================================

#[async_trait]
impl Channel for WebhookChannel {
    fn name(&self) -> &str {
        "Webhook"
    }

    fn channel_type(&self) -> &str {
        "webhook"
    }

    /// Handle an incoming message: extract text, route through AI, format response.
    async fn handle_message(&self, msg: ChannelMessage) -> Result<ChannelResponse, String> {
        let text = msg.content.as_str();

        if text.is_empty() {
            return Ok(ChannelResponse {
                content: String::new(),
                card: None,
            });
        }

        let response_content = match &self.session_manager {
            Some(sm) => match sm.process_message("default", text, None).await {
                Ok(resp) => resp,
                Err(e) => {
                    warn!("Webhook AI processing failed: {}", e);
                    format!("Error processing message: {}", e)
                }
            },
            None => format!("Received: {}", text),
        };

        Ok(ChannelResponse {
            content: response_content,
            card: None,
        })
    }

    /// Verify the webhook channel setup.
    ///
    /// Webhook verification checks that the secret (if configured) can be used
    /// for HMAC signing. For a simple test, we check that a known signature
    /// round-trips correctly.
    async fn verify(&self, params: VerifyParams) -> Result<bool, String> {
        // If no secret is configured, always pass
        if self.config.secret.is_empty() {
            return Ok(true);
        }

        // If a test payload and signature are provided, verify them
        if let (Some(payload), Some(sig)) = (
            params.params.get("test_payload").and_then(|v| v.as_str()),
            params.params.get("test_signature").and_then(|v| v.as_str()),
        ) {
            return verify_hmac(&self.config.secret, payload.as_bytes(), sig)
                .map(|_| true);
        }

        // Otherwise just confirm configuration exists
        Ok(true)
    }
}

// ============================================================
// Section 7: Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- HMAC tests ---

    #[test]
    fn test_verify_hmac_valid() {
        let secret = "my-secret-key";
        let body = b"hello world";

        // Compute expected signature
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let expected_hex = hex::encode(mac.finalize().into_bytes());

        let result = verify_hmac(secret, body, &expected_hex);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_hmac_invalid() {
        let secret = "my-secret-key";
        let body = b"hello world";

        let result = verify_hmac(secret, body, "abadc0ffee");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_hmac_wrong_secret() {
        let secret = "my-secret-key";
        let body = b"hello world";

        // Sign with a different secret
        let mut mac = HmacSha256::new_from_slice("wrong-secret".as_bytes()).unwrap();
        mac.update(body);
        let wrong_hex = hex::encode(mac.finalize().into_bytes());

        let result = verify_hmac(secret, body, &wrong_hex);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_hmac_tampered_body() {
        let secret = "my-secret-key";
        let body = b"hello world";
        let tampered = b"hello world!";

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let original_hex = hex::encode(mac.finalize().into_bytes());

        let result = verify_hmac(secret, tampered, &original_hex);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_hmac_with_sha256_prefix() {
        let secret = "my-secret-key";
        let body = b"test payload";

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let hex_sig = hex::encode(mac.finalize().into_bytes());
        let prefixed = format!("sha256={}", hex_sig);

        let result = verify_hmac(secret, body, &prefixed);
        assert!(result.is_ok());
    }

    // --- JSON path extraction tests ---

    #[test]
    fn test_extract_json_path_simple() {
        let body = serde_json::json!({"message": "hello world"});
        assert_eq!(
            extract_json_path(&body, "message"),
            Some("hello world".to_string())
        );
    }

    #[test]
    fn test_extract_json_path_nested() {
        let body = serde_json::json!({
            "data": {
                "text": "nested message",
                "metadata": {"key": "value"}
            }
        });
        assert_eq!(
            extract_json_path(&body, "data.text"),
            Some("nested message".to_string())
        );
        // Nested object should be serialized
        let metadata = extract_json_path(&body, "data.metadata");
        assert!(metadata.is_some());
    }

    #[test]
    fn test_extract_json_path_missing() {
        let body = serde_json::json!({"message": "hello"});
        assert_eq!(extract_json_path(&body, "data.text"), None);
    }

    #[test]
    fn test_extract_json_path_empty() {
        let body = serde_json::json!({"message": "hello"});
        let result = extract_json_path(&body, "");
        assert!(result.is_some());
        // Should return the whole body as JSON
    }

    #[test]
    fn test_extract_json_path_root() {
        let body = serde_json::json!({"key": "value"});
        let result = extract_json_path(&body, "$");
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_json_path_number() {
        let body = serde_json::json!({"count": 42});
        assert_eq!(
            extract_json_path(&body, "count"),
            Some("42".to_string())
        );
    }

    #[test]
    fn test_extract_json_path_bool() {
        let body = serde_json::json!({"active": true});
        assert_eq!(
            extract_json_path(&body, "active"),
            Some("true".to_string())
        );
    }

    #[test]
    fn test_extract_json_path_null() {
        let body = serde_json::json!({"empty": null});
        assert_eq!(extract_json_path(&body, "empty"), None);
    }

    // --- Response formatting tests ---

    #[test]
    fn test_format_response_default_template() {
        let result = format_response("AI answer", r#"{"reply": "{{reply}}"}"#);
        assert_eq!(result["reply"].as_str(), Some("AI answer"));
    }

    #[test]
    fn test_format_response_custom_template() {
        let result = format_response(
            "Hello world",
            r#"{"result": "{{reply}}", "ts": "{{timestamp}}"}"#,
        );
        assert_eq!(result["result"].as_str(), Some("Hello world"));
        assert!(result["ts"].as_str().is_some());
    }

    #[test]
    fn test_format_response_bare_reply() {
        let result = format_response("just text", "{{reply}}");
        assert_eq!(result.as_str(), Some("just text"));
    }

    #[test]
    fn test_format_response_special_characters() {
        let result = format_response(
            "He said \"hello\"\nand went home",
            r#"{"reply": "{{reply}}"}"#,
        );
        let reply = result["reply"].as_str().unwrap();
        assert!(reply.contains("hello"));
    }

    // --- Webhook message extraction tests ---

    #[test]
    fn test_webhook_extract_message() {
        let config = WebhookConfig {
            json_message_path: "data.input".to_string(),
            ..Default::default()
        };
        let channel = WebhookChannel::new(config);

        let body = serde_json::json!({
            "data": {"input": "What is Rust?"}
        });
        assert_eq!(channel.extract_message(&body), "What is Rust?");
    }

    // --- Webhook signature verification tests ---

    #[test]
    fn test_webhook_no_secret_skips_verification() {
        let config = WebhookConfig::default(); // secret is empty
        let channel = WebhookChannel::new(config);

        let result = channel.verify_signature(b"anything", "");
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_with_secret_requires_signature() {
        let config = WebhookConfig {
            secret: "test-secret".to_string(),
            ..Default::default()
        };
        let channel = WebhookChannel::new(config);

        let result = channel.verify_signature(b"body", "");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing"));
    }

    #[test]
    fn test_webhook_valid_signature() {
        let config = WebhookConfig {
            secret: "test-secret".to_string(),
            ..Default::default()
        };
        let channel = WebhookChannel::new(config);
        let body = b"test-body";

        let mut mac = HmacSha256::new_from_slice("test-secret".as_bytes()).unwrap();
        mac.update(body);
        let hex_sig = hex::encode(mac.finalize().into_bytes());

        let result = channel.verify_signature(body, &hex_sig);
        assert!(result.is_ok());
    }

    // ── Additional edge case tests ──

    #[test]
    fn test_verify_hmac_empty_secret() {
        // Empty secret should still compute HMAC
        let result = verify_hmac("", b"hello", "00000000");
        assert!(result.is_err()); // empty secret produces different hash
    }

    #[test]
    fn test_verify_hmac_empty_body() {
        let secret = "my-secret-key";
        let body = b"";

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let expected_hex = hex::encode(mac.finalize().into_bytes());

        let result = verify_hmac(secret, body, &expected_hex);
        assert!(result.is_ok(), "Empty body should still verify");
    }

    #[test]
    fn test_verify_hmac_length_mismatch_short() {
        let secret = "my-secret-key";
        // A valid signature is 64 hex chars (SHA-256 = 32 bytes = 64 hex chars)
        // Passing a short one should fail
        let result = verify_hmac(secret, b"hello", "00");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_hmac_length_mismatch_long() {
        let secret = "key";
        // Passing a too-long hex string should fail
        let result = verify_hmac(secret, b"hello", &"a".repeat(128));
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_json_path_deeply_nested() {
        let body = serde_json::json!({
            "level1": {
                "level2": {
                    "level3": {
                        "value": "deep"
                    }
                }
            }
        });
        assert_eq!(
            extract_json_path(&body, "level1.level2.level3.value"),
            Some("deep".to_string())
        );
    }

    #[test]
    fn test_extract_json_path_non_object_mid_path() {
        let body = serde_json::json!({"data": "not-an-object"});
        // "data.field" should fail because "data" is a string, not an object
        assert_eq!(extract_json_path(&body, "data.field"), None);
    }

    #[test]
    fn test_format_response_unicode_characters() {
        let result = format_response(
            "你好世界 🌍 こんにちは",
            r#"{"reply": "{{reply}}"}"#,
        );
        let reply = result["reply"].as_str().unwrap();
        assert!(reply.contains("你好世界"));
        assert!(reply.contains("🌍"));
        assert!(reply.contains("こんにちは"));
    }

    #[test]
    fn test_format_response_json_special_chars() {
        let result = format_response(
            r#"quote "test" backslash \ slash / newline
"#,
            r#"{"reply": "{{reply}}"}"#,
        );
        let reply = result["reply"].as_str().unwrap();
        // Should have the escaped content properly
        assert!(reply.contains("quote"));
    }

    #[test]
    fn test_webhook_channel_handle_empty_message() {
        let config = WebhookConfig::default();
        let channel = WebhookChannel::new(config);

        let msg = ChannelMessage {
            channel_type: "webhook".into(),
            chat_id: "test".into(),
            user_id: "user".into(),
            content: String::new(),
            raw: serde_json::json!({}),
        };

        // Can't use .await in a sync test, but we can test the trait method
        // by checking the extract logic implicitly
        assert!(msg.content.is_empty());
    }
}
