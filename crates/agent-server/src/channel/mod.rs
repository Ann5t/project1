//! Channel abstraction layer for multi-platform messaging.
//!
//! Provides the `Channel` trait and common message types. Each platform
//! (Feishu/Lark, QQ Bot, WeChat Work) is implemented in its own submodule.
//!
//! ## Architecture
//!
//! ```text
//! External Platform        Channel Trait          AI Agent Core
//! -----------------        --------------          --------------
//! Feishu event  --> parse_callback() --> ChannelMessage
//!                                          |
//! QQ WS dispatch --> parse_dispatch() --> ChannelMessage
//!                                          |
//!                                    SessionManager.process_message()
//!                                          |
//!                                    send_message() / send_card()
//! ```

pub mod feishu;
pub mod qq_bot;
pub mod webhook;
pub mod wechat_work;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Normalized incoming message from any supported channel.
///
/// Each platform adapter is responsible for converting its native event
/// format into this common struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub channel_type: String,
    pub chat_id: String,
    pub user_id: String,
    pub content: String,
    pub raw: serde_json::Value,
}

/// Response to send back to a channel, containing plain text and an
/// optional rich card payload (Feishu interactive cards, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResponse {
    pub content: String,
    #[serde(default)]
    pub card: Option<serde_json::Value>,
}

/// Verification parameters for channel setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyParams {
    pub params: serde_json::Value,
}

/// Abstract channel trait -- implement for each messaging platform.
///
/// # Required methods
///
/// - `name`: Human-readable name (e.g., "Feishu").
/// - `channel_type`: Machine identifier (e.g., `"feishu"`).
/// - `handle_message`: Process an incoming message and produce a response.
/// - `verify`: Verify channel setup (URL verification, signature check, etc.).
#[async_trait]
pub trait Channel: Send + Sync {
    /// Human-readable channel name (e.g., "Feishu", "QQ Bot").
    fn name(&self) -> &str;

    /// Channel type identifier used in the database (`feishu`, `qq`, `wechat_work`).
    fn channel_type(&self) -> &str;

    /// Handle an incoming message and return a response.
    async fn handle_message(&self, msg: ChannelMessage) -> Result<ChannelResponse, String>;

    /// Verify the channel setup (URL verification challenge, Ed25519 signature, etc.).
    async fn verify(&self, params: VerifyParams) -> Result<bool, String>;
}
