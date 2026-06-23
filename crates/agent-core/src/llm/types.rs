//! LLM request/response types for the OpenAI-compatible chat completions API.
//!
//! These types are used by [`LlmClient`](super::client::LlmClient) implementations
//! and map directly to the standard chat completion JSON schema.

use serde::{Deserialize, Serialize};

/// A single message in a chat conversation.
///
/// Roles must be one of: `system`, `user`, `assistant`, `tool`.
/// `tool_calls` is set when the assistant requests function/tool calls.
/// `tool_call_id` is set for tool-result messages that reply to a specific call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// A request to the LLM chat completion API.
///
/// Maps to the OpenAI-compatible `/v1/chat/completions` request body.
/// Set `stream: true` for streaming responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

/// A single choice in a chat completion response.
///
/// Each choice contains a `message` and an optional `finish_reason`
/// (`"stop"`, `"length"`, `"tool_calls"`, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

/// Complete chat completion response from the LLM API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
}

/// A tool definition sent to the LLM for function calling.
///
/// Wraps a [`FunctionDefinition`] with the type field set to `"function"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub r#type: String,
    pub function: FunctionDefinition,
}

/// Function definition containing name, description, and JSON Schema parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// A tool/function call requested by the LLM in its response.
///
/// Contains the `id` (used to match tool-result messages), the `type`
/// field (always `"function"`), and the `function` details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: ToolCallFunction,
}

/// Details of a specific function call: the function `name` and JSON `arguments`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// Result of executing a tool, keyed by `tool_call_id` so the LLM can match
/// the result to the original call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
}

/// A single SSE streaming delta chunk from the LLM.
///
/// Contains the text `delta`, optional `finish_reason`, and optional
/// `tool_calls` (for streaming function-calling responses).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDelta {
    pub delta: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ChatMessage Serialization ──

    #[test]
    fn chat_message_serialize_basic() {
        let msg = ChatMessage {
            role: "user".into(),
            content: "Hello, world!".into(),
            tool_calls: None,
            tool_call_id: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let expected = r#"{"role":"user","content":"Hello, world!"}"#;
        assert_eq!(json, expected);
    }

    #[test]
    fn chat_message_deserialize_basic() {
        let json = r#"{"role":"assistant","content":"Hi there!"}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "Hi there!");
        assert!(msg.tool_calls.is_none());
        assert!(msg.tool_call_id.is_none());
    }

    #[test]
    fn chat_message_with_tool_calls() {
        let msg = ChatMessage {
            role: "assistant".into(),
            content: "".into(),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".into(),
                r#type: "function".into(),
                function: ToolCallFunction {
                    name: "calculator".into(),
                    arguments: r#"{"expression":"2+2"}"#.into(),
                },
            }]),
            tool_call_id: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("tool_calls"));
        assert!(json.contains("call_1"));
        assert!(json.contains("calculator"));

        // Round-trip
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.role, "assistant");
        let tc = back.tool_calls.unwrap();
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].id, "call_1");
        assert_eq!(tc[0].function.name, "calculator");
    }

    #[test]
    fn chat_message_with_tool_call_id() {
        let msg = ChatMessage {
            role: "tool".into(),
            content: "Result: 4".into(),
            tool_calls: None,
            tool_call_id: Some("call_tool_abc".into()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("tool_call_id"));
        assert!(json.contains("call_tool_abc"));

        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.role, "tool");
        assert_eq!(back.tool_call_id.unwrap(), "call_tool_abc");
    }

    // ── ChatRequest Serialization ──

    #[test]
    fn chat_request_serialize_minimal() {
        let req = ChatRequest {
            model: "deepseek-chat".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "ping".into(),
                tool_calls: None,
                tool_call_id: None,
            }],
            tools: None,
            temperature: None,
            max_tokens: None,
            stream: false,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""model":"deepseek-chat""#));
        assert!(json.contains(r#""stream":false"#));
        // Optional fields should be absent
        assert!(!json.contains("temperature"));
        assert!(!json.contains("max_tokens"));
        assert!(!json.contains("tools"));
    }

    #[test]
    fn chat_request_serialize_full() {
        let req = ChatRequest {
            model: "gpt-4".into(),
            messages: vec![ChatMessage {
                role: "system".into(),
                content: "You are helpful.".into(),
                tool_calls: None,
                tool_call_id: None,
            }],
            tools: Some(vec![serde_json::json!({
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get weather",
                    "parameters": {"type": "object", "properties": {}}
                }
            })]),
            temperature: Some(0.5),
            max_tokens: Some(2048),
            stream: true,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""temperature":0.5"#));
        assert!(json.contains(r#""max_tokens":2048"#));
        assert!(json.contains(r#""stream":true"#));
        assert!(json.contains("get_weather"));

        // Round-trip
        let back: ChatRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.model, "gpt-4");
        assert_eq!(back.messages.len(), 1);
        assert_eq!(back.temperature.unwrap(), 0.5);
        assert_eq!(back.max_tokens.unwrap(), 2048);
        assert!(back.stream);
        assert!(back.tools.is_some());
    }

    #[test]
    fn chat_request_deserialize_from_llm_api_format() {
        // Simulate a realistic API request format
        let json = r#"{
            "model": "deepseek-chat",
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "What is 2+2?"}
            ],
            "stream": false
        }"#;
        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "deepseek-chat");
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, "system");
        assert_eq!(req.messages[1].content, "What is 2+2?");
        assert!(!req.stream);
    }

    // ── ChatResponse Serialization ──

    #[test]
    fn chat_response_deserialize_realistic() {
        let json = r#"{
            "id": "chatcmpl-abc123",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "deepseek-chat",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "2+2 equals 4."
                    },
                    "finish_reason": "stop"
                }
            ]
        }"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "chatcmpl-abc123");
        assert_eq!(resp.object, "chat.completion");
        assert_eq!(resp.model, "deepseek-chat");
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].index, 0);
        assert_eq!(resp.choices[0].message.role, "assistant");
        assert_eq!(resp.choices[0].message.content, "2+2 equals 4.");
        assert_eq!(resp.choices[0].finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn chat_response_serialize_roundtrip() {
        let resp = ChatResponse {
            id: "test-id".into(),
            object: "chat.completion".into(),
            created: 1234567890,
            model: "test-model".into(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "Hello".into(),
                    tool_calls: None,
                    tool_call_id: None,
                },
                finish_reason: Some("stop".into()),
            }],
        };

        let json = serde_json::to_string(&resp).unwrap();
        let back: ChatResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, resp.id);
        assert_eq!(back.model, resp.model);
        assert_eq!(back.choices.len(), 1);
        assert_eq!(back.choices[0].finish_reason, resp.choices[0].finish_reason);
    }

    // ── ToolCall and ToolCallFunction ──

    #[test]
    fn tool_call_serialize() {
        let tc = ToolCall {
            id: "call_xyz".into(),
            r#type: "function".into(),
            function: ToolCallFunction {
                name: "get_current_time".into(),
                arguments: r#"{}"#.into(),
            },
        };

        let json = serde_json::to_string(&tc).unwrap();
        assert!(json.contains(r#""id":"call_xyz""#));
        assert!(json.contains(r#""type":"function""#));
        assert!(json.contains("get_current_time"));

        let back: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "call_xyz");
        assert_eq!(back.function.name, "get_current_time");
    }

    #[test]
    fn tool_call_deserialize_from_llm_response() {
        let json = r#"{
            "id": "call_123",
            "type": "function",
            "function": {
                "name": "web_search",
                "arguments": "{\"query\":\"Rust programming\"}"
            }
        }"#;
        let tc: ToolCall = serde_json::from_str(json).unwrap();
        assert_eq!(tc.id, "call_123");
        assert_eq!(tc.r#type, "function");
        assert_eq!(tc.function.name, "web_search");
        assert!(tc.function.arguments.contains("Rust programming"));
    }

    // ── ToolDefinition and FunctionDefinition ──

    #[test]
    fn tool_definition_serialize() {
        let td = ToolDefinition {
            r#type: "function".into(),
            function: FunctionDefinition {
                name: "calculator".into(),
                description: "Evaluate math".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "expression": {"type": "string"}
                    },
                    "required": ["expression"]
                }),
            },
        };

        let json = serde_json::to_string(&td).unwrap();
        assert!(json.contains(r#""type":"function""#));
        assert!(json.contains(r#""name":"calculator""#));

        let back: ToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(back.function.name, "calculator");
        assert_eq!(back.function.description, "Evaluate math");
    }

    // ── ToolResult ──

    #[test]
    fn tool_result_serialize() {
        let tr = ToolResult {
            tool_call_id: "call_1".into(),
            content: "Result: 42".into(),
        };

        let json = serde_json::to_string(&tr).unwrap();
        assert!(json.contains("call_1"));
        assert!(json.contains("Result: 42"));

        let back: ToolResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tool_call_id, "call_1");
        assert_eq!(back.content, "Result: 42");
    }

    // ── StreamDelta ──

    #[test]
    fn stream_delta_serialize_basic() {
        let sd = StreamDelta {
            delta: "Hello".into(),
            finish_reason: None,
            tool_calls: None,
        };

        let json = serde_json::to_string(&sd).unwrap();
        assert!(json.contains(r#""delta":"Hello""#));
        // finish_reason should be absent
        assert!(!json.contains("finish_reason"));

        let back: StreamDelta = serde_json::from_str(&json).unwrap();
        assert_eq!(back.delta, "Hello");
        assert!(back.finish_reason.is_none());
    }

    #[test]
    fn stream_delta_serialize_with_finish() {
        let sd = StreamDelta {
            delta: String::new(),
            finish_reason: Some("stop".into()),
            tool_calls: None,
        };

        let json = serde_json::to_string(&sd).unwrap();
        assert!(json.contains(r#""finish_reason":"stop""#));
    }

    #[test]
    fn stream_delta_with_tool_calls() {
        let sd = StreamDelta {
            delta: String::new(),
            finish_reason: Some("tool_calls".into()),
            tool_calls: Some(vec![ToolCall {
                id: "t1".into(),
                r#type: "function".into(),
                function: ToolCallFunction {
                    name: "calculator".into(),
                    arguments: r#"{"expression":"1+1"}"#.into(),
                },
            }]),
        };

        let json = serde_json::to_string(&sd).unwrap();
        assert!(json.contains("tool_calls"));
        assert!(json.contains("calculator"));

        let back: StreamDelta = serde_json::from_str(&json).unwrap();
        let tc = back.tool_calls.unwrap();
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].function.name, "calculator");
    }

    // ── Edge Cases ──

    #[test]
    fn empty_messages_array() {
        let json = r#"{"model":"test","messages":[],"stream":false}"#;
        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert!(req.messages.is_empty());
    }

    #[test]
    fn message_with_special_characters() {
        let msg = ChatMessage {
            role: "user".into(),
            content: "Line 1\nLine 2\tTabbed\n\"Quoted\"".into(),
            tool_calls: None,
            tool_call_id: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.content, msg.content);
    }

    #[test]
    fn message_with_unicode() {
        let msg = ChatMessage {
            role: "user".into(),
            content: "你好世界 🌍".into(),
            tool_calls: None,
            tool_call_id: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.content, "你好世界 🌍");
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let json = r#"{
            "role": "user",
            "content": "test",
            "unknown_field": 42,
            "also_unknown": null
        }"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "test");
    }

    #[test]
    fn multiple_choices_response() {
        let json = r#"{
            "id": "x",
            "object": "chat.completion",
            "created": 1,
            "model": "test",
            "choices": [
                {"index": 0, "message": {"role": "assistant", "content": "First"}, "finish_reason": null},
                {"index": 1, "message": {"role": "assistant", "content": "Second"}, "finish_reason": null}
            ]
        }"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 2);
        assert_eq!(resp.choices[1].message.content, "Second");
    }
}
