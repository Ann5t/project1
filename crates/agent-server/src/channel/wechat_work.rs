//! WeChat Work (企业微信) Bot channel integration
//!
//! Supports the WeChat Work (企业微信) self-built application message API.
//! Reference: https://developer.work.weixin.qq.com/document/path/90236
//!
//! ## Architecture
//!
//! 1. **Token Management** -- Cached access token with 2-hour expiry (5-min buffer).
//! 2. **URL Verification** -- GET callback: decrypt `echostr` using AES-256-CBC to prove
//!    ownership of the EncodingAESKey.
//! 3. **Message Receiving** -- POST callback: decrypt the XML-encrypted payload, parse
//!    the inner XML message (text / image / event types).
//! 4. **Message Sending** -- Active push via `message/send` API: text, markdown, and
//!    news (图文) article cards.

use std::sync::Arc;
use std::time::{Duration, Instant};

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use async_trait::async_trait;
use base64::Engine;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::{Channel, ChannelMessage, ChannelResponse, VerifyParams};

// ============================================================
// Section 1: AES-256-CBC Cipher & Crypto Helpers
// ============================================================

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// WeChat Work message encryption/decryption protocol.
///
/// # Protocol layout (after decrypt / before encrypt)
///
/// ```text
/// | random (16 B) | msg_len (4B BE) | msg (msg_len B) | receiveid (corpid, variable) |
/// ```
///
/// The AES-256 key is the Base64-decoded EncodingAESKey (43 chars -> 32 bytes).
/// The IV is the first 16 bytes of the AES key.
struct WxCrypt {
    aes_key: [u8; 32],
    iv: [u8; 16],
    receiveid: String,
}

impl WxCrypt {
    /// Build the crypto engine from the 43-character Base64 `encoding_aes_key`
    /// and the `corp_id` used for the protocol trailer.
    fn new(encoding_aes_key: &str, receiveid: &str) -> Result<Self, String> {
        // EncodingAESKey is 43 chars, pad to "=" for standard Base64.
        let padded = format!("{}=", encoding_aes_key);
        let key_bytes = base64::engine::general_purpose::STANDARD
            .decode(&padded)
            .map_err(|e| format!("Invalid EncodingAESKey (base64 decode): {}", e))?;

        if key_bytes.len() != 32 {
            return Err(format!(
                "EncodingAESKey must decode to 32 bytes, got {}",
                key_bytes.len()
            ));
        }

        let mut aes_key = [0u8; 32];
        aes_key.copy_from_slice(&key_bytes);

        let mut iv = [0u8; 16];
        iv.copy_from_slice(&aes_key[..16]);

        Ok(Self {
            aes_key,
            iv,
            receiveid: receiveid.to_string(),
        })
    }

    /// Decrypt a Base64-encoded ciphertext from a WeChat Work callback.
    ///
    /// Returns the plaintext message bytes (after stripping random prefix,
    /// length header, and receiveid trailer).
    fn decrypt(&self, encrypted_base64: &str) -> Result<String, String> {
        let cipher_bytes = base64::engine::general_purpose::STANDARD
            .decode(encrypted_base64)
            .map_err(|e| format!("Base64 decode failed: {}", e))?;

        // Decrypt with AES-256-CBC (PKCS7 padding)
        let mut buf = cipher_bytes;
        let plain = Aes256CbcDec::new(&self.aes_key.into(), &self.iv.into())
            .decrypt_padded_mut::<Pkcs7>(&mut buf)
            .map_err(|e| format!("AES decrypt failed: {}", e))?;

        // Parse wire format: random(16) + msg_len(4 BE) + msg + receiveid
        if plain.len() < 20 {
            return Err("Decrypted payload too short".to_string());
        }

        let msg_len = u32::from_be_bytes(
            plain[16..20]
                .try_into()
                .map_err(|_| "Failed to read msg_len".to_string())?,
        ) as usize;

        let msg_start = 20;
        let msg_end = msg_start + msg_len;

        if plain.len() < msg_end {
            return Err(format!(
                "Message truncated: expected {msg_end} bytes, got {}",
                plain.len()
            ));
        }

        let msg = &plain[msg_start..msg_end];
        let receiveid_from = &plain[msg_end..];

        // Verify receiveid
        if receiveid_from != self.receiveid.as_bytes() {
            return Err(format!(
                "receiveid mismatch: expected '{}', got '{}'",
                self.receiveid,
                String::from_utf8_lossy(receiveid_from)
            ));
        }

        String::from_utf8(msg.to_vec()).map_err(|e| format!("UTF-8 decode: {}", e))
    }

    /// Encrypt a plaintext message into a Base64 ciphertext for sending
    /// (used for URL verification echostr echo and passive reply).
    #[allow(dead_code)]
    fn encrypt(&self, msg: &str) -> Result<String, String> {
        let msg_bytes = msg.as_bytes();
        let msg_len = msg_bytes.len() as u32;
        let receiveid_bytes = self.receiveid.as_bytes();

        // random(16) + msg_len(4 BE) + msg + receiveid
        let data_len = 16 + 4 + msg_bytes.len() + receiveid_bytes.len();
        let mut plain = vec![0u8; data_len];
        rand::thread_rng().fill(&mut plain[..16]);
        plain[16..20].copy_from_slice(&msg_len.to_be_bytes());
        plain[20..20 + msg_bytes.len()].copy_from_slice(msg_bytes);
        plain[20 + msg_bytes.len()..].copy_from_slice(receiveid_bytes);

        // Make room for PKCS7 padding (always at least 1 byte, up to 16)
        let block_size: usize = 16;
        let pad_len = block_size - (data_len % block_size);
        plain.resize(data_len + pad_len, 0);

        // AES-256-CBC encrypt in-place with PKCS7 padding
        let ciphertext = cbc::Encryptor::<aes::Aes256>::new(&self.aes_key.into(), &self.iv.into())
            .encrypt_padded_mut::<Pkcs7>(&mut plain, data_len)
            .map_err(|e| format!("AES encrypt failed: {}", e))?;

        Ok(base64::engine::general_purpose::STANDARD.encode(ciphertext))
    }
}

/// Compute the WeChat Work signature: SHA1(sort(token, timestamp, nonce, encrypt)).
fn compute_signature(token: &str, timestamp: &str, nonce: &str, encrypt: &str) -> String {
    let mut items = [token, timestamp, nonce, encrypt];
    items.sort_unstable();
    let joined = items.join("");
    let mut hasher = Sha1::new();
    hasher.update(joined.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ============================================================
// Section 2: WeChat Work XML Message Types
// ============================================================

/// Parsed WeChat Work text message from XML callback.
#[derive(Debug, Clone)]
pub struct WxTextMessage {
    pub to_user: String,
    pub from_user: String,
    pub create_time: i64,
    pub content: String,
    pub msg_id: i64,
    pub agent_id: i32,
}

/// Parsed WeChat Work image message from XML callback.
#[derive(Debug, Clone)]
pub struct WxImageMessage {
    pub to_user: String,
    pub from_user: String,
    pub create_time: i64,
    pub pic_url: String,
    pub media_id: String,
    pub msg_id: i64,
    pub agent_id: i32,
}

/// Parsed WeChat Work event message from XML callback.
#[derive(Debug, Clone)]
pub struct WxEventMessage {
    pub to_user: String,
    pub from_user: String,
    pub create_time: i64,
    pub event: String, // "subscribe", "unsubscribe", "click", etc.
    pub event_key: Option<String>,
    pub agent_id: i32,
}

/// Decrypted XML envelope from WeChat Work callback.
///
/// WeChat Work wraps the encrypted payload in XML:
/// ```xml
/// <xml>
///   <ToUserName><![CDATA[...]]></ToUserName>
///   <Encrypt><![CDATA[...]]></Encrypt>
///   <AgentID><![CDATA[...]]></AgentID>
/// </xml>
/// ```
#[derive(Debug, Clone)]
pub struct WxCallbackEnvelope {
    pub to_user_name: String,
    pub encrypt: String,
    pub agent_id: Option<String>,
}

/// Parse the encrypted XML envelope from the callback body.
fn parse_callback_envelope(xml_body: &str) -> Result<WxCallbackEnvelope, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml_body);
    reader.config_mut().trim_text(true);

    let mut to_user_name = String::new();
    let mut encrypt = String::new();
    let mut agent_id = None;
    let mut current_tag = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
            }
            Ok(Event::Text(ref e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                match current_tag.as_str() {
                    "ToUserName" => to_user_name = text,
                    "Encrypt" => encrypt = text,
                    "AgentID" => agent_id = Some(text),
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
    }

    if encrypt.is_empty() {
        return Err("No Encrypt field found in callback XML".to_string());
    }

    Ok(WxCallbackEnvelope {
        to_user_name,
        encrypt,
        agent_id,
    })
}

/// Parse a decrypted message XML string into a typed variant.
#[derive(Debug, Clone)]
pub enum WxMessage {
    Text(WxTextMessage),
    Image(WxImageMessage),
    Event(WxEventMessage),
    Unknown { msg_type: String, raw_xml: String },
}

impl WxMessage {
    /// Parse from decrypted XML string.
    pub fn parse(xml_str: &str) -> Result<Self, String> {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut reader = Reader::from_str(xml_str);
        reader.config_mut().trim_text(true);

        let mut fields: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let mut current_tag = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                }
                Ok(Event::Text(ref e)) => {
                    let text = String::from_utf8_lossy(e.as_ref()).to_string();
                    if !current_tag.is_empty() {
                        fields.insert(current_tag.clone(), text);
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(format!("XML parse error: {}", e)),
                _ => {}
            }
        }

        let msg_type = fields.get("MsgType").cloned().unwrap_or_default();

        match msg_type.as_str() {
            "text" => Ok(WxMessage::Text(WxTextMessage {
                to_user: fields.get("ToUserName").cloned().unwrap_or_default(),
                from_user: fields.get("FromUserName").cloned().unwrap_or_default(),
                create_time: fields
                    .get("CreateTime")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                content: fields.get("Content").cloned().unwrap_or_default(),
                msg_id: fields
                    .get("MsgId")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                agent_id: fields
                    .get("AgentID")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            })),
            "image" => Ok(WxMessage::Image(WxImageMessage {
                to_user: fields.get("ToUserName").cloned().unwrap_or_default(),
                from_user: fields.get("FromUserName").cloned().unwrap_or_default(),
                create_time: fields
                    .get("CreateTime")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                pic_url: fields.get("PicUrl").cloned().unwrap_or_default(),
                media_id: fields.get("MediaId").cloned().unwrap_or_default(),
                msg_id: fields
                    .get("MsgId")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                agent_id: fields
                    .get("AgentID")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            })),
            "event" => Ok(WxMessage::Event(WxEventMessage {
                to_user: fields.get("ToUserName").cloned().unwrap_or_default(),
                from_user: fields.get("FromUserName").cloned().unwrap_or_default(),
                create_time: fields
                    .get("CreateTime")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                event: fields.get("Event").cloned().unwrap_or_default(),
                event_key: fields.get("EventKey").cloned(),
                agent_id: fields
                    .get("AgentID")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            })),
            other => Ok(WxMessage::Unknown {
                msg_type: other.to_string(),
                raw_xml: xml_str.to_string(),
            }),
        }
    }
}

// ============================================================
// Section 3: WeChat Work API Response Types
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WxApiError {
    pub errcode: i32,
    pub errmsg: String,
}

/// Token response from `gettoken`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WxTokenResponse {
    pub errcode: i32,
    pub errmsg: Option<String>,
    pub access_token: Option<String>,
    pub expires_in: Option<i64>,
}

/// Response from `message/send`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WxSendResponse {
    pub errcode: i32,
    pub errmsg: Option<String>,
    pub invaliduser: Option<String>,
    pub invalidparty: Option<String>,
    pub invalidtag: Option<String>,
}

/// WeChat Work error code lookup (for user-friendly messages).
fn wx_error_message(errcode: i32) -> String {
    match errcode {
        -1 => "System busy, please retry".into(),
        0 => "OK".into(),
        40001 => "Invalid credential (corpsecret mismatch or access_token expired)".into(),
        40003 => "Invalid UserID".into(),
        40005 => "Invalid file type".into(),
        40007 => "Invalid media_id".into(),
        40008 => "Invalid message type".into(),
        40013 => "Invalid CorpID".into(),
        40014 => "Invalid access_token".into(),
        41001 => "Missing access_token parameter".into(),
        41002 => "Missing CorpID parameter".into(),
        41003 => "Missing CorpSecret parameter".into(),
        42001 => "access_token expired".into(),
        42007 => "Token authentication failed".into(),
        43004 => "User not subscribed to the agent (需要接收消息的用户需要先关注该应用)".into(),
        45001 => "Media file not found".into(),
        45002 => "Message content exceeds limit".into(),
        45007 => "Voice playback time exceeded limit".into(),
        45008 => "Article count exceeds limit".into(),
        45009 => "API call frequency limit exceeded".into(),
        47001 => "Message body parse error".into(),
        48001 => "API function not authorized".into(),
        48002 => "User not allowed to use this agent".into(),
        50001 => "User redirected (redirect_uri not authorized)".into(),
        60001 => "Department ID not found".into(),
        60011 => "Invalid department member count configuration".into(),
        82001 => "No agents in this department".into(),
        82002 => "Agent info not found".into(),
        82003 => "URL parameter error (verify URL or Token)".into(),
        86001 => "Chat session not exist".into(),
        86003 => "Chat session not exist (non-group)".into(),
        86004 => "Invalid chat type".into(),
        86201 => "Chat not exist".into(),
        86202 => "Chat parameter error".into(),
        86204 => "New user created successfully".into(),
        _ => format!("Unknown WeChat Work error (code: {errcode})"),
    }
}

/// Check the errcode in a JSON response and return an error string on failure.
fn check_wx_response(errcode: i32, errmsg: Option<&str>) -> Result<(), String> {
    if errcode != 0 {
        let msg = errmsg.unwrap_or("unknown");
        let detail = wx_error_message(errcode);
        Err(format!(
            "WeChat Work API error {}: {} -- {}",
            errcode, msg, detail
        ))
    } else {
        Ok(())
    }
}

// ============================================================
// Section 4: WechatWorkChannel
// ============================================================

/// WeChat Work Bot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WechatWorkConfig {
    /// Corp ID from WeChat Work admin console
    pub corp_id: String,
    /// Corp Secret for the self-built application
    pub corp_secret: String,
    /// Application Agent ID
    pub agent_id: String,
    /// Token for callback URL verification (10+ chars)
    pub token: String,
    /// Encoding AES Key for message encryption/decryption (43 chars)
    pub encoding_aes_key: String,
}

/// WeChat Work channel implementation.
///
/// Handles callback verification, message decryption, parsing, and sending
/// responses via the WeChat Work message API.
pub struct WechatWorkChannel {
    pub config: WechatWorkConfig,
    http_client: reqwest::Client,
    wx_crypt: WxCrypt,
    /// Cached access token and its expiry timestamp
    token_cache: RwLock<Option<(String, Instant)>>,
    /// Shared session manager for AI processing
    pub session_manager: Option<Arc<agent_core::session::manager::SessionManager>>,
}

impl WechatWorkChannel {
    /// Create a new WeChat Work channel from its configuration.
    ///
    /// Returns an error if the `encoding_aes_key` is malformed.
    pub fn new(config: WechatWorkConfig) -> Result<Self, String> {
        let wx_crypt = WxCrypt::new(&config.encoding_aes_key, &config.corp_id)?;
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(5)
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to build WeChat Work HTTP client with custom settings: {e}. Falling back to default.");
                reqwest::Client::new()
            });
        Ok(Self {
            config,
            http_client,
            wx_crypt,
            token_cache: RwLock::new(None),
            session_manager: None,
        })
    }

    /// Attach a SessionManager for AI-powered message handling.
    pub fn with_session_manager(
        mut self,
        sm: Arc<agent_core::session::manager::SessionManager>,
    ) -> Self {
        self.session_manager = Some(sm);
        self
    }

    // ---- Token Management ----

    /// Get an access token, refreshing from the API when the cache is stale.
    ///
    /// Tokens expire after 7200 seconds (2 hours). We cache with a 5-minute
    /// safety buffer to avoid using nearly-expired tokens.
    pub async fn get_access_token(&self) -> Result<String, String> {
        // Check cache first (with 5-min buffer)
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
            .get(&format!(
                "https://qyapi.weixin.qq.com/cgi-bin/gettoken?corpid={}&corpsecret={}",
                self.config.corp_id, self.config.corp_secret
            ))
            .send()
            .await
            .map_err(|e| format!("WeChat Work token request failed: {}", e))?;

        let body: WxTokenResponse = resp
            .json()
            .await
            .map_err(|e| format!("WeChat Work token parse error: {}", e))?;

        check_wx_response(body.errcode, body.errmsg.as_deref())?;

        let token = body
            .access_token
            .ok_or_else(|| "No access_token in response".to_string())?;

        let expire_secs = body.expires_in.unwrap_or(7200).max(300) as u64;

        // Cache with 5-min safety margin
        let mut cache = self.token_cache.write().await;
        *cache = Some((
            token.clone(),
            Instant::now() + Duration::from_secs(expire_secs.saturating_sub(300)),
        ));

        debug!(
            "WeChat Work access token refreshed, expires in {}s",
            expire_secs
        );
        Ok(token)
    }

    // ---- URL Verification ----

    /// Verify the callback URL during WeChat Work application setup.
    ///
    /// Called on `GET /api/channels/wechat_work/callback?msg_signature=...&timestamp=...&nonce=...&echostr=...`
    ///
    /// Steps:
    /// 1. Verify the SHA1 signature
    /// 2. Decrypt the `echostr` with AES-256-CBC
    /// 3. Return the decrypted echostr as the response body
    pub fn verify_url(
        &self,
        msg_signature: &str,
        timestamp: &str,
        nonce: &str,
        echostr: &str,
    ) -> Result<String, String> {
        // 1. Verify signature
        let expected_sig = compute_signature(&self.config.token, timestamp, nonce, echostr);
        if expected_sig != msg_signature {
            return Err(format!(
                "Signature verification failed: expected '{}', got '{}'",
                expected_sig, msg_signature
            ));
        }

        // 2. Decrypt echostr
        let plain = self.wx_crypt.decrypt(echostr)?;
        info!("WeChat Work URL verification succeeded");
        Ok(plain)
    }

    // ---- Message Decryption & Parsing ----

    /// Parse an incoming callback message.
    ///
    /// The callback is an HTTP POST with an XML body containing the encrypted
    /// payload. This method:
    ///
    /// 1. Verifies the signature
    /// 2. Parses the XML envelope
    /// 3. Decrypts the payload
    /// 4. Parses the inner XML message
    /// 5. Returns a typed `WxMessage`
    pub fn parse_callback(
        &self,
        body: &str,
        msg_signature: &str,
        timestamp: &str,
        nonce: &str,
    ) -> Result<(WxCallbackEnvelope, WxMessage), String> {
        // 1. Parse the outer XML envelope
        let envelope = parse_callback_envelope(body)?;

        // 2. Verify signature
        let expected_sig =
            compute_signature(&self.config.token, timestamp, nonce, &envelope.encrypt);
        if expected_sig != msg_signature {
            return Err(format!(
                "Signature verification failed: expected '{}', got '{}'",
                expected_sig, msg_signature
            ));
        }

        // 3. Decrypt the payload
        let decrypted_xml = self.wx_crypt.decrypt(&envelope.encrypt)?;
        debug!(
            "Decrypted WeChat Work message ({} chars)",
            decrypted_xml.len()
        );

        // 4. Parse the inner message XML
        let message = WxMessage::parse(&decrypted_xml)?;

        Ok((envelope, message))
    }

    // ---- Message Sending ----

    /// Send a text message to one or more users.
    ///
    /// `to_user` can be a single UserID, multiple UserIDs separated by "|",
    /// "@all" for everyone, or a department/party ID.
    pub async fn send_text_message(&self, to_user: &str, content: &str) -> Result<(), String> {
        let token = self.get_access_token().await?;

        let body = serde_json::json!({
            "touser": to_user,
            "msgtype": "text",
            "agentid": self.config.agent_id.parse::<i32>().unwrap_or(0),
            "text": {
                "content": content
            },
            "safe": 0
        });

        let resp = self
            .http_client
            .post(&format!(
                "https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={}",
                token
            ))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("WeChat Work send failed: {}", e))?;

        let result: WxSendResponse = resp
            .json()
            .await
            .map_err(|e| format!("WeChat Work send parse error: {}", e))?;

        check_wx_response(result.errcode, result.errmsg.as_deref())?;

        if let Some(ref invalid) = result.invaliduser {
            if !invalid.is_empty() {
                warn!("WeChat Work message: invalid users: {}", invalid);
            }
        }

        info!("Sent text message to WeChat Work user(s): {}", to_user);
        Ok(())
    }

    /// Send a markdown message to one or more users.
    pub async fn send_markdown(&self, to_user: &str, content: &str) -> Result<(), String> {
        let token = self.get_access_token().await?;

        let body = serde_json::json!({
            "touser": to_user,
            "msgtype": "markdown",
            "agentid": self.config.agent_id.parse::<i32>().unwrap_or(0),
            "markdown": {
                "content": content
            }
        });

        let resp = self
            .http_client
            .post(&format!(
                "https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={}",
                token
            ))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("WeChat Work markdown send failed: {}", e))?;

        let result: WxSendResponse = resp
            .json()
            .await
            .map_err(|e| format!("WeChat Work markdown parse error: {}", e))?;

        check_wx_response(result.errcode, result.errmsg.as_deref())?;

        info!("Sent markdown message to WeChat Work user(s): {}", to_user);
        Ok(())
    }

    /// Send a news (图文) article card message to one or more users.
    ///
    /// Each article entry contains: title, description, url, and optional picurl (cover image).
    pub async fn send_news(&self, to_user: &str, articles: &[NewsArticle]) -> Result<(), String> {
        if articles.is_empty() {
            return Err("No articles to send".to_string());
        }
        if articles.len() > 8 {
            return Err("News articles limited to 8 per message".to_string());
        }

        let token = self.get_access_token().await?;

        let articles_json: Vec<serde_json::Value> = articles
            .iter()
            .map(|a| {
                let mut j = serde_json::json!({
                    "title": a.title,
                    "description": a.description,
                    "url": a.url,
                });
                if let Some(ref picurl) = a.picurl {
                    j["picurl"] = serde_json::Value::String(picurl.clone());
                }
                j
            })
            .collect();

        let body = serde_json::json!({
            "touser": to_user,
            "msgtype": "news",
            "agentid": self.config.agent_id.parse::<i32>().unwrap_or(0),
            "news": {
                "articles": articles_json
            }
        });

        let resp = self
            .http_client
            .post(&format!(
                "https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={}",
                token
            ))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("WeChat Work news send failed: {}", e))?;

        let result: WxSendResponse = resp
            .json()
            .await
            .map_err(|e| format!("WeChat Work news parse error: {}", e))?;

        check_wx_response(result.errcode, result.errmsg.as_deref())?;

        info!(
            "Sent {} news article(s) to WeChat Work user(s): {}",
            articles.len(),
            to_user
        );
        Ok(())
    }

    /// Build a news card from an AI response text.
    ///
    /// Splits long text into a formatted card with a truncated preview.
    pub fn build_ai_response_articles(response_text: &str) -> Vec<NewsArticle> {
        // Truncate for description
        let description = if response_text.len() > 200 {
            format!("{}...", &response_text[..200])
        } else {
            response_text.to_string()
        };

        // For code-heavy responses, we can split into multiple articles
        let title = if response_text.contains("```") {
            "AI Response (with code)".to_string()
        } else {
            "AI Response".to_string()
        };

        vec![NewsArticle {
            title,
            description,
            url: String::new(),
            picurl: None,
        }]
    }
}

/// A news (图文) article entry for WeChat Work news message type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsArticle {
    /// Article title (required, max ~128 chars)
    pub title: String,
    /// Article description/summary (optional, max ~512 chars)
    pub description: String,
    /// Click-through URL (required, or empty for no link)
    pub url: String,
    /// Cover image URL (optional, JPG/PNG, 150x150 recommended)
    #[serde(default)]
    pub picurl: Option<String>,
}

// ── ChannelMessage conversion helpers ──

impl WechatWorkChannel {
    /// Convert a parsed `WxMessage` into a normalized `ChannelMessage`.
    pub fn to_channel_message(&self, wx_msg: &WxMessage) -> ChannelMessage {
        match wx_msg {
            WxMessage::Text(t) => ChannelMessage {
                channel_type: "wechat_work".to_string(),
                chat_id: t.from_user.clone(), // DM: chat_id = from_user
                user_id: t.from_user.clone(),
                content: t.content.clone(),
                raw: serde_json::json!({
                    "to_user": t.to_user,
                    "from_user": t.from_user,
                    "create_time": t.create_time,
                    "msg_type": "text",
                    "content": t.content,
                    "msg_id": t.msg_id,
                    "agent_id": t.agent_id,
                }),
            },
            WxMessage::Image(img) => {
                // For image: we use the media_id as content for OCR stub
                let content = format!(
                    "[Image] pic_url: {}\nmedia_id: {}\n(OCR processing not available)",
                    img.pic_url, img.media_id
                );
                ChannelMessage {
                    channel_type: "wechat_work".to_string(),
                    chat_id: img.from_user.clone(),
                    user_id: img.from_user.clone(),
                    content,
                    raw: serde_json::json!({
                        "to_user": img.to_user,
                        "from_user": img.from_user,
                        "msg_type": "image",
                        "pic_url": img.pic_url,
                        "media_id": img.media_id,
                        "msg_id": img.msg_id,
                        "agent_id": img.agent_id,
                    }),
                }
            }
            WxMessage::Event(evt) => {
                let event_desc = match evt.event.as_str() {
                    "subscribe" => "User just subscribed to the agent".to_string(),
                    "unsubscribe" => "User unsubscribed from the agent".to_string(),
                    "click" => format!(
                        "User clicked menu: key={}",
                        evt.event_key.as_deref().unwrap_or("?")
                    ),
                    "enter_agent" => "User entered the agent chat".to_string(),
                    _ => format!("Unknown event: {}", evt.event),
                };
                ChannelMessage {
                    channel_type: "wechat_work".to_string(),
                    chat_id: evt.from_user.clone(),
                    user_id: evt.from_user.clone(),
                    content: event_desc,
                    raw: serde_json::json!({
                        "to_user": evt.to_user,
                        "from_user": evt.from_user,
                        "msg_type": "event",
                        "event": evt.event,
                        "event_key": evt.event_key,
                        "agent_id": evt.agent_id,
                    }),
                }
            }
            WxMessage::Unknown { msg_type, raw_xml } => ChannelMessage {
                channel_type: "wechat_work".to_string(),
                chat_id: "unknown".to_string(),
                user_id: "unknown".to_string(),
                content: format!("Unsupported message type: {}", msg_type),
                raw: serde_json::json!({
                    "msg_type": msg_type,
                    "raw_xml": raw_xml,
                }),
            },
        }
    }
}

// ============================================================
// Section 5: Channel Trait Implementation
// ============================================================

#[async_trait]
impl Channel for WechatWorkChannel {
    fn name(&self) -> &str {
        "企业微信"
    }

    fn channel_type(&self) -> &str {
        "wechat_work"
    }

    async fn handle_message(&self, msg: ChannelMessage) -> Result<ChannelResponse, String> {
        // For direct ChannelMessage handling (non-callback path), simply echo
        let text = msg.content.as_str();

        // If a session manager is available, process through AI
        let response_content = match &self.session_manager {
            Some(sm) => match sm.process_message("default", text, None).await {
                Ok(resp) => resp,
                Err(e) => {
                    warn!("AI processing failed (handle_message): {}", e);
                    format!("Received: {}", text)
                }
            },
            None => format!("Received: {}", text),
        };

        Ok(ChannelResponse {
            content: response_content,
            card: None,
        })
    }

    /// Verify the channel configuration.
    ///
    /// For WeChat Work, verification parameters include:
    /// - `msg_signature`: SHA1 signature
    /// - `timestamp`: Unix timestamp
    /// - `nonce`: Random string
    /// - `echostr`: Encrypted verification string
    async fn verify(&self, params: VerifyParams) -> Result<bool, String> {
        let msg_sig = params
            .params
            .get("msg_signature")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let timestamp = params
            .params
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let nonce = params
            .params
            .get("nonce")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let echostr = params
            .params
            .get("echostr")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if echostr.is_empty() {
            return Ok(false);
        }

        match self.verify_url(msg_sig, timestamp, nonce, echostr) {
            Ok(plain) => {
                info!(
                    "WeChat Work URL verification succeeded. Echostr: {}",
                    &plain[..plain.len().min(30)]
                );
                Ok(true)
            }
            Err(e) => {
                error!("WeChat Work URL verification failed: {}", e);
                Err(e)
            }
        }
    }
}
