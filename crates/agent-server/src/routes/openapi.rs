//! OpenAPI 3.0 specification endpoint and Swagger UI documentation page.
//!
//! - `GET /api/openapi.json` -- Full OpenAPI 3.0 spec as JSON
//! - `GET /api/docs` -- Dark-themed Swagger UI HTML page

use axum::response::Html;
use axum::Json;
use serde_json::{json, Value};
use std::sync::LazyLock;

/// Pre-built OpenAPI 3.0 specification.
///
/// Constructed once at startup via `LazyLock`, then served
/// from every call to `GET /api/openapi.json`.
static OPENAPI_SPEC: LazyLock<Value> = LazyLock::new(build_spec);

fn build_spec() -> Value {
    let version = env!("CARGO_PKG_VERSION");

    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "AI Agent API",
            "version": version,
            "description": "REST API for the AI Agent application. Manages sessions, chat, channels, workflows, scheduled tasks, monitoring, export, backup, and publishing.\n\nAll API routes (except health, auth status, channel callbacks, and published pages) require authentication via a Bearer token when `auth_enabled` is true.",
            "contact": {
                "name": "AI Agent Team"
            },
            "license": {
                "name": "MIT"
            }
        },
        "servers": [
            {
                "url": "http://localhost:{port}",
                "description": "Local development server",
                "variables": {
                    "port": {
                        "default": "3000"
                    }
                }
            }
        ],
        "tags": [
            {"name": "Health", "description": "Health check and system information endpoints"},
            {"name": "Auth", "description": "Authentication status and token login"},
            {"name": "Config", "description": "Key-value configuration management"},
            {"name": "Sessions", "description": "Conversation session CRUD and messages"},
            {"name": "Chat", "description": "Message sending (non-streaming and SSE streaming)"},
            {"name": "Channels", "description": "Channel management and platform callbacks"},
            {"name": "Workflows", "description": "Workflow definition CRUD and execution"},
            {"name": "Tasks", "description": "Scheduled task CRUD, execution, and logs"},
            {"name": "Search", "description": "Search sessions and messages"},
            {"name": "Export", "description": "Session and workflow data export"},
            {"name": "Backup", "description": "Database backup, restore, and listing"},
            {"name": "Monitor", "description": "Monitoring statistics, dashboard, and counters reset"},
            {"name": "Notifications", "description": "Email notification testing"},
            {"name": "Publish", "description": "Published workflow result pages"}
        ],
        "paths": {
            "/api/health": {
                "get": {
                    "tags": ["Health"],
                    "summary": "Health check",
                    "description": "Returns server status with version and current UTC timestamp. No authentication required.",
                    "operationId": "healthCheck",
                    "security": [],
                    "responses": {
                        "200": {
                            "description": "Server is healthy",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/HealthResponse"},
                                    "example": {
                                        "status": "ok",
                                        "version": "0.1.0",
                                        "timestamp": "2026-06-19T12:00:00+00:00"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/info": {
                "get": {
                    "tags": ["Health"],
                    "summary": "System information",
                    "description": "Returns server version, counts of sessions/channels/workflows/tasks/tools, supported channels, and feature flags. No authentication required.",
                    "operationId": "systemInfo",
                    "security": [],
                    "responses": {
                        "200": {
                            "description": "System information",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/InfoResponse"},
                                    "example": {
                                        "version": "0.1.0",
                                        "status": "running",
                                        "timestamp": "2026-06-19T12:00:00+00:00",
                                        "stats": {
                                            "sessions": 5,
                                            "active_channels": 2,
                                            "workflows": 3,
                                            "enabled_tasks": 1,
                                            "tools": 5
                                        },
                                        "tools": ["calculator", "current_time", "web_search", "read_file", "execute_shell"],
                                        "channels": ["feishu", "qq", "wechat_work", "webhook"],
                                        "features": {
                                            "chat": true,
                                            "streaming": true,
                                            "workflows": true,
                                            "scheduler": true,
                                            "publishing": true,
                                            "onboarding": true
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/auth/status": {
                "get": {
                    "tags": ["Auth"],
                    "summary": "Get authentication status",
                    "description": "Returns whether token-based authentication is enabled and whether an admin token has been configured. No authentication required.",
                    "operationId": "authStatus",
                    "security": [],
                    "responses": {
                        "200": {
                            "description": "Authentication status",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/AuthStatusResponse"},
                                    "example": {
                                        "auth_enabled": true,
                                        "configured": true
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/auth/login": {
                "post": {
                    "tags": ["Auth"],
                    "summary": "Login with token",
                    "description": "Validates an authentication token. Returns session info on success or 401 on mismatch. No authentication required.",
                    "operationId": "authLogin",
                    "security": [],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/LoginRequest"},
                                "example": {
                                    "token": "550e8400-e29b-41d4-a716-446655440000"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Login successful",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/LoginResponse"},
                                    "example": {
                                        "valid": true,
                                        "token": "550e8400-e29b-41d4-a716-446655440000",
                                        "message": "Login successful"
                                    }
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"}
                    }
                }
            },
            "/api/config": {
                "get": {
                    "tags": ["Config"],
                    "summary": "List all configuration",
                    "description": "Returns all configuration entries as a flat key-value map.",
                    "operationId": "configGetAll",
                    "responses": {
                        "200": {
                            "description": "Configuration key-value map",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ConfigMap"},
                                    "example": {
                                        "admin_token": "550e8400-...",
                                        "auth_enabled": "true",
                                        "api_key": "sk-...",
                                        "rate_limit_global_rpm": "100",
                                        "smtp_enabled": "false"
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "put": {
                    "tags": ["Config"],
                    "summary": "Batch update configuration",
                    "description": "Upserts multiple configuration keys at once. Accepts a flat JSON object of key-value pairs.",
                    "operationId": "configUpdateAll",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/ConfigMap"},
                                "example": {
                                    "auth_enabled": "true",
                                    "rate_limit_global_rpm": "200"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Configuration updated",
                            "content": {
                                "application/json": {
                                    "example": {"updated": true}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/config/{key}": {
                "get": {
                    "tags": ["Config"],
                    "summary": "Get a single config value",
                    "description": "Returns a single configuration value by key. Returns 404 if the key is not found.",
                    "operationId": "configGetOne",
                    "parameters": [
                        {"$ref": "#/components/parameters/KeyParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Configuration value",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ConfigValueResponse"},
                                    "example": {
                                        "key": "auth_enabled",
                                        "value": "true"
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "put": {
                    "tags": ["Config"],
                    "summary": "Set a single config value",
                    "description": "Sets or updates a single configuration value. The body must contain a `value` field.",
                    "operationId": "configSetOne",
                    "parameters": [
                        {"$ref": "#/components/parameters/KeyParam"}
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["value"],
                                    "properties": {
                                        "value": {"type": "string", "description": "The value to set for this key"}
                                    }
                                },
                                "example": {
                                    "value": "true"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Configuration set",
                            "content": {
                                "application/json": {
                                    "example": {"updated": true}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/sessions": {
                "get": {
                    "tags": ["Sessions"],
                    "summary": "List all sessions",
                    "description": "Returns all conversation sessions ordered by most recently updated.",
                    "operationId": "sessionList",
                    "responses": {
                        "200": {
                            "description": "List of sessions",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": {"$ref": "#/components/schemas/SessionRow"}
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "post": {
                    "tags": ["Sessions"],
                    "summary": "Create a new session",
                    "description": "Creates a new conversation session. Supports optional system_prompt, model, temperature, and max_tokens. Defaults: model=`deepseek-chat`, temperature=0.7, max_tokens=4096, channel=`web`.",
                    "operationId": "sessionCreate",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/CreateSessionRequest"},
                                "example": {
                                    "name": "My Chat Session",
                                    "agent_id": "assistant",
                                    "system_prompt": "You are a helpful assistant.",
                                    "model": "deepseek-chat",
                                    "temperature": 0.7,
                                    "max_tokens": 4096
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Created session",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/SessionRow"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/sessions/{id}": {
                "get": {
                    "tags": ["Sessions"],
                    "summary": "Get a session by ID",
                    "description": "Returns a single session by ID. Returns 404 if not found.",
                    "operationId": "sessionGetOne",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Session details",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/SessionRow"}
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "put": {
                    "tags": ["Sessions"],
                    "summary": "Update a session",
                    "description": "Updates session fields. All fields are optional. Returns 404 if the session ID is not found.",
                    "operationId": "sessionUpdate",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/UpdateSessionRequest"},
                                "example": {
                                    "name": "Updated Chat Name",
                                    "temperature": 0.9
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Updated session",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/SessionRow"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "delete": {
                    "tags": ["Sessions"],
                    "summary": "Delete a session",
                    "description": "Deletes a session and all its associated messages (cascade).",
                    "operationId": "sessionDelete",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Session deleted",
                            "content": {
                                "application/json": {
                                    "example": {"deleted": true}
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/sessions/{id}/messages": {
                "get": {
                    "tags": ["Sessions"],
                    "summary": "Get session messages",
                    "description": "Returns all messages for a session ordered by creation time.",
                    "operationId": "sessionMessages",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "List of messages",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": {"$ref": "#/components/schemas/MessageRow"}
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/chat": {
                "post": {
                    "tags": ["Chat"],
                    "summary": "Send a chat message",
                    "description": "Sends a message and returns the full AI response. Auto-creates a session when no `session_id` is provided. Subject to chat rate limiting.",
                    "operationId": "chatSendMessage",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/ChatRequest"},
                                "example": {
                                    "session_id": "550e8400-e29b-41d4-a716-446655440000",
                                    "message": "Hello, how are you?",
                                    "model": "deepseek-chat"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "AI response",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ChatResponse"},
                                    "example": {
                                        "session_id": "550e8400-e29b-41d4-a716-446655440000",
                                        "message": "Hello! I'm doing well, thank you for asking. How can I help you today?"
                                    }
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "429": {"$ref": "#/components/responses/RateLimited"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/chat/stream": {
                "post": {
                    "tags": ["Chat"],
                    "summary": "Stream a chat message (SSE)",
                    "description": "Sends a message and receives a Server-Sent Events stream with JSON events discriminated by `type`: `thinking`, `delta`, `tool_start`, `tool_end`, `done`, `error`. Subject to chat rate limiting and SSE concurrency cap.",
                    "operationId": "chatStreamMessage",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/ChatRequest"},
                                "example": {
                                    "session_id": "550e8400-e29b-41d4-a716-446655440000",
                                    "message": "Explain quantum computing",
                                    "model": "deepseek-chat"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "SSE stream of response events",
                            "content": {
                                "text/event-stream": {
                                    "schema": {"$ref": "#/components/schemas/SseEvent"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "429": {"$ref": "#/components/responses/RateLimited"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/channels": {
                "get": {
                    "tags": ["Channels"],
                    "summary": "List all channels",
                    "description": "Returns all configured channels with their current status and configuration.",
                    "operationId": "channelList",
                    "responses": {
                        "200": {
                            "description": "List of channels",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": {"$ref": "#/components/schemas/ChannelRow"}
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "post": {
                    "tags": ["Channels"],
                    "summary": "Create a new channel",
                    "description": "Creates a new channel with the given type, name, and JSON config. The channel is disabled by default; use PUT to enable.",
                    "operationId": "channelCreate",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/CreateChannelRequest"},
                                "example": {
                                    "channel_type": "feishu",
                                    "name": "My Feishu Bot",
                                    "config": {
                                        "app_id": "cli_xxx",
                                        "app_secret": "..."
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Created channel",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ChannelRow"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/channels/{id}": {
                "put": {
                    "tags": ["Channels"],
                    "summary": "Update a channel",
                    "description": "Updates an existing channel's name, enabled status, or config. All fields optional.",
                    "operationId": "channelUpdate",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/UpdateChannelRequest"},
                                "example": {
                                    "enabled": true,
                                    "name": "Updated Bot Name"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Updated channel",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ChannelRow"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "delete": {
                    "tags": ["Channels"],
                    "summary": "Delete a channel",
                    "description": "Deletes a channel by ID.",
                    "operationId": "channelDelete",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Channel deleted",
                            "content": {
                                "application/json": {
                                    "example": {"deleted": true}
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/channels/{id}/test": {
                "post": {
                    "tags": ["Channels"],
                    "summary": "Test a channel connection",
                    "description": "Tests the connection for a specific channel. Implementation depends on channel type.",
                    "operationId": "channelTest",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Test result",
                            "content": {
                                "application/json": {
                                    "example": {
                                        "channel_id": "550e8400-...",
                                        "status": "ok",
                                        "message": "Channel test not yet implemented"
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/channels/feishu/callback": {
                "post": {
                    "tags": ["Channels"],
                    "summary": "Feishu event subscription callback",
                    "description": "Handles Feishu/Lark event subscription callbacks: URL verification (challenge echo), event decryption, message parsing, AI processing, and response delivery. No authentication required.",
                    "operationId": "feishuCallback",
                    "security": [],
                    "responses": {
                        "200": {
                            "description": "Feishu callback acknowledged",
                            "content": {
                                "application/json": {
                                    "example": {"code": 0, "msg": "ok"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"}
                    }
                }
            },
            "/api/channels/wechat_work/callback": {
                "get": {
                    "tags": ["Channels"],
                    "summary": "WeChat Work URL verification",
                    "description": "Handles WeChat Work URL verification. Expects query parameters: msg_signature, timestamp, nonce, echostr. Returns decrypted echostr. No authentication required.",
                    "operationId": "wechatWorkVerify",
                    "security": [],
                    "parameters": [
                        {"$ref": "#/components/parameters/MsgSignatureParam"},
                        {"$ref": "#/components/parameters/TimestampParam"},
                        {"$ref": "#/components/parameters/NonceParam"},
                        {"$ref": "#/components/parameters/EchostrParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Decrypted echostr (plain text)",
                            "content": {
                                "text/plain": {}
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"}
                    }
                },
                "post": {
                    "tags": ["Channels"],
                    "summary": "WeChat Work message callback",
                    "description": "Receives and processes encrypted WeChat Work message events. No authentication required.",
                    "operationId": "wechatWorkCallback",
                    "security": [],
                    "parameters": [
                        {"$ref": "#/components/parameters/MsgSignatureParam"},
                        {"$ref": "#/components/parameters/TimestampParam"},
                        {"$ref": "#/components/parameters/NonceParam"}
                    ],
                    "requestBody": {
                        "content": {
                            "text/xml": {
                                "description": "Encrypted XML payload from WeChat Work"
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Acknowledgement (always returns 'success')",
                            "content": {
                                "text/plain": {}
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"}
                    }
                }
            },
            "/api/channels/webhook/{path}": {
                "post": {
                    "tags": ["Channels"],
                    "summary": "Generic webhook callback",
                    "description": "Receives JSON payloads from external services. Extracts a message, optionally verifies HMAC-SHA256 signature, processes through AI, and returns a formatted response. No authentication required.",
                    "operationId": "webhookCallback",
                    "security": [],
                    "parameters": [
                        {
                            "name": "path",
                            "in": "path",
                            "required": true,
                            "schema": {"type": "string"},
                            "description": "Webhook URL path segment matching the configured webhook"
                        }
                    ],
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "description": "Any valid JSON object. The message is extracted using the configured json_message_path."
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Formatted AI response",
                            "content": {
                                "application/json": {
                                    "example": {
                                        "reply": "This is the AI's response to your webhook message."
                                    }
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/workflows": {
                "get": {
                    "tags": ["Workflows"],
                    "summary": "List all workflows",
                    "description": "Returns all workflow definitions ordered by most recently updated.",
                    "operationId": "workflowList",
                    "responses": {
                        "200": {
                            "description": "List of workflows",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": {"$ref": "#/components/schemas/WorkflowRow"}
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "post": {
                    "tags": ["Workflows"],
                    "summary": "Create a new workflow",
                    "description": "Creates a new workflow definition. Trigger type can be `manual` or `cron` (requires cron_expression).",
                    "operationId": "workflowCreate",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/CreateWorkflowRequest"},
                                "example": {
                                    "name": "Daily Report",
                                    "description": "Generate a daily summary report",
                                    "trigger_type": "cron",
                                    "cron_expression": "0 9 * * *",
                                    "definition": {
                                        "steps": [
                                            {"id": "fetch", "type": "http", "config": {"url": "https://api.example.com/data"}},
                                            {"id": "summarize", "type": "llm", "config": {"prompt": "Summarize the data"}}
                                        ],
                                        "edges": [{"from": "fetch", "to": "summarize"}]
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Created workflow",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/WorkflowRow"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/workflows/{id}": {
                "get": {
                    "tags": ["Workflows"],
                    "summary": "Get a workflow by ID",
                    "description": "Returns a single workflow definition by ID. Returns 404 if not found.",
                    "operationId": "workflowGetOne",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Workflow details",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/WorkflowRow"}
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "put": {
                    "tags": ["Workflows"],
                    "summary": "Update a workflow",
                    "description": "Updates workflow fields. All fields optional. The definition field, if provided, replaces the entire DAG.",
                    "operationId": "workflowUpdate",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/UpdateWorkflowRequest"},
                                "example": {
                                    "enabled": false,
                                    "name": "Updated Report Workflow"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Updated workflow",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/WorkflowRow"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "delete": {
                    "tags": ["Workflows"],
                    "summary": "Delete a workflow",
                    "description": "Deletes a workflow and its execution records (cascade).",
                    "operationId": "workflowDelete",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Workflow deleted",
                            "content": {
                                "application/json": {
                                    "example": {"deleted": true}
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/workflows/{id}/run": {
                "post": {
                    "tags": ["Workflows"],
                    "summary": "Execute a workflow immediately",
                    "description": "Parses the workflow definition as a DAG, topologically sorts it, and executes in parallel batches. Results include per-step status and output.",
                    "operationId": "workflowRun",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Workflow execution result",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/WorkflowRunResult"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/workflows/{id}/runs": {
                "get": {
                    "tags": ["Workflows"],
                    "summary": "List workflow execution history",
                    "description": "Returns execution history for a workflow. Accepts `?limit=N` (default 20).",
                    "operationId": "workflowRuns",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"},
                        {"$ref": "#/components/parameters/LimitQuery"}
                    ],
                    "responses": {
                        "200": {
                            "description": "List of workflow runs",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": {"$ref": "#/components/schemas/WorkflowRunRow"}
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/tasks": {
                "get": {
                    "tags": ["Tasks"],
                    "summary": "List all scheduled tasks",
                    "description": "Returns all scheduled tasks ordered by most recently updated.",
                    "operationId": "taskList",
                    "responses": {
                        "200": {
                            "description": "List of tasks",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": {"$ref": "#/components/schemas/ScheduledTaskRow"}
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "post": {
                    "tags": ["Tasks"],
                    "summary": "Create a new scheduled task",
                    "description": "Creates a new scheduled task. Requires name, cron_expression (5-field format), and prompt. Defaults: model=`deepseek-chat`, enabled=`true`.",
                    "operationId": "taskCreate",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/CreateTaskRequest"},
                                "example": {
                                    "name": "Morning Briefing",
                                    "cron_expression": "0 8 * * *",
                                    "prompt": "Generate a morning briefing based on current news",
                                    "model": "deepseek-chat",
                                    "enabled": true
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Created task",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ScheduledTaskRow"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/tasks/{id}": {
                "get": {
                    "tags": ["Tasks"],
                    "summary": "Get a task by ID",
                    "description": "Returns a single scheduled task by ID. Returns 404 if not found.",
                    "operationId": "taskGetOne",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Task details",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ScheduledTaskRow"}
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "put": {
                    "tags": ["Tasks"],
                    "summary": "Update a task",
                    "description": "Updates task fields. All fields optional. Changing the cron expression takes effect on the next scheduler tick.",
                    "operationId": "taskUpdate",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/UpdateTaskRequest"},
                                "example": {
                                    "cron_expression": "0 9 * * *",
                                    "enabled": false
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Updated task",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/ScheduledTaskRow"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                },
                "delete": {
                    "tags": ["Tasks"],
                    "summary": "Delete a task",
                    "description": "Deletes a task and its execution logs (cascade).",
                    "operationId": "taskDelete",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Task deleted",
                            "content": {
                                "application/json": {
                                    "example": {"deleted": true}
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/tasks/{id}/run": {
                "post": {
                    "tags": ["Tasks"],
                    "summary": "Run a task immediately",
                    "description": "Triggers a task execution immediately, bypassing the cron schedule. Returns the LLM response.",
                    "operationId": "taskRunNow",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Task execution result",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/TaskRunResponse"},
                                    "example": {
                                        "task_id": "550e8400-...",
                                        "result": "Good morning! Here is your daily briefing..."
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/tasks/{id}/logs": {
                "get": {
                    "tags": ["Tasks"],
                    "summary": "Get task execution logs",
                    "description": "Returns the last 50 execution log entries for a task, ordered by most recent first.",
                    "operationId": "taskLogs",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "List of task logs",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": {"$ref": "#/components/schemas/TaskLogRow"}
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/search": {
                "get": {
                    "tags": ["Search"],
                    "summary": "Search sessions and messages",
                    "description": "Searches sessions by name and messages by content using SQL LIKE. Returns ranked results with relevance snippets and supports pagination.",
                    "operationId": "search",
                    "parameters": [
                        {"$ref": "#/components/parameters/SearchQueryParam"},
                        {"$ref": "#/components/parameters/SearchTypeParam"},
                        {"$ref": "#/components/parameters/PageParam"},
                        {"$ref": "#/components/parameters/LimitQuery"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Search results",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/SearchResponse"}
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/export/session/{id}": {
                "get": {
                    "tags": ["Export"],
                    "summary": "Export a session",
                    "description": "Exports a full conversation session in one of three formats: json (application/json), markdown (text/markdown), or html (text/html) with dark-themed styling.",
                    "operationId": "exportSession",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"},
                        {"$ref": "#/components/parameters/ExportFormatParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Exported session",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "exported_at": {"type": "string", "format": "date-time"},
                                            "session": {"$ref": "#/components/schemas/SessionRow"},
                                            "messages": {
                                                "type": "array",
                                                "items": {"$ref": "#/components/schemas/MessageRow"}
                                            },
                                            "message_count": {"type": "integer"}
                                        }
                                    }
                                },
                                "text/markdown": {
                                    "schema": {"type": "string"}
                                },
                                "text/html": {
                                    "schema": {"type": "string"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/export/workflow/{id}/runs": {
                "get": {
                    "tags": ["Export"],
                    "summary": "Export workflow runs",
                    "description": "Exports all workflow execution history as JSON or CSV.",
                    "operationId": "exportWorkflowRuns",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"},
                        {
                            "name": "format",
                            "in": "query",
                            "required": false,
                            "schema": {
                                "type": "string",
                                "enum": ["json", "csv"],
                                "default": "json"
                            },
                            "description": "Export format"
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Exported workflow runs",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "exported_at": {"type": "string", "format": "date-time"},
                                            "workflow_id": {"type": "string"},
                                            "workflow_name": {"type": "string"},
                                            "runs": {
                                                "type": "array",
                                                "items": {"$ref": "#/components/schemas/WorkflowRunRow"}
                                            },
                                            "run_count": {"type": "integer"}
                                        }
                                    }
                                },
                                "text/csv": {
                                    "schema": {"type": "string"}
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/export/bulk": {
                "post": {
                    "tags": ["Export"],
                    "summary": "Bulk export sessions",
                    "description": "Exports multiple sessions at once, returned as a zip file containing one file per session. Limited to 50 sessions per request.",
                    "operationId": "exportBulk",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/BulkExportRequest"},
                                "example": {
                                    "session_ids": [
                                        "550e8400-e29b-41d4-a716-446655440000",
                                        "660e8400-e29b-41d4-a716-446655440001"
                                    ],
                                    "format": "json"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Zip archive of exported sessions",
                            "content": {
                                "application/zip": {
                                    "schema": {
                                        "type": "string",
                                        "format": "binary"
                                    }
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "404": {"$ref": "#/components/responses/NotFound"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/backup": {
                "get": {
                    "tags": ["Backup"],
                    "summary": "Create a database backup",
                    "description": "Uses SQLite VACUUM INTO to create a clean backup copy. Returns the backup file as an application/octet-stream download.",
                    "operationId": "backupCreate",
                    "responses": {
                        "200": {
                            "description": "Database backup file",
                            "content": {
                                "application/octet-stream": {
                                    "schema": {
                                        "type": "string",
                                        "format": "binary"
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/backup/restore": {
                "post": {
                    "tags": ["Backup"],
                    "summary": "Restore database from backup",
                    "description": "Restores the database from an uploaded .db file via multipart form-data. Validates the SQLite header before restoring. Performed atomically within a transaction.",
                    "operationId": "backupRestore",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "required": ["file"],
                                    "properties": {
                                        "file": {
                                            "type": "string",
                                            "format": "binary",
                                            "description": "SQLite .db backup file (field name: 'file' or 'backup')"
                                        }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Restore result",
                            "content": {
                                "application/json": {
                                    "example": {
                                        "status": "restored",
                                        "message": "Database has been restored from the uploaded backup.",
                                        "file": "agent-backup-20260619.db",
                                        "size_bytes": 1_024_000,
                                        "tables_restored": true
                                    }
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/backup/list": {
                "get": {
                    "tags": ["Backup"],
                    "summary": "List backup files",
                    "description": "Returns all backup files in the data/backups/ directory with name, size, and last-modified timestamp, sorted by most recent first.",
                    "operationId": "backupList",
                    "responses": {
                        "200": {
                            "description": "List of backup entries",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "backups": {
                                                "type": "array",
                                                "items": {"$ref": "#/components/schemas/BackupEntry"}
                                            },
                                            "count": {"type": "integer"},
                                            "directory": {"type": "string"}
                                        }
                                    },
                                    "example": {
                                        "backups": [
                                            {
                                                "filename": "agent-backup-20260619-120000.db",
                                                "size_bytes": 1_024_000,
                                                "modified": "2026-06-19T12:00:00+00:00"
                                            }
                                        ],
                                        "count": 1,
                                        "directory": "data/backups"
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/monitor": {
                "get": {
                    "tags": ["Monitor"],
                    "summary": "Get monitoring statistics",
                    "description": "Returns full server monitoring stats: uptime, request count, active SSE/WS connections, database size, memory usage, LLM API stats, per-channel message counts, and recent errors.",
                    "operationId": "monitorGet",
                    "responses": {
                        "200": {
                            "description": "Monitoring statistics",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/MonitorStats"}
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/api/monitor/reset": {
                "post": {
                    "tags": ["Monitor"],
                    "summary": "Reset monitoring counters",
                    "description": "Resets all runtime counters (request count, SSE/WS connections, per-channel messages, LLM stats) and clears the recent error log.",
                    "operationId": "monitorReset",
                    "responses": {
                        "200": {
                            "description": "Reset confirmation",
                            "content": {
                                "application/json": {
                                    "example": {
                                        "status": "ok",
                                        "message": "All monitoring counters have been reset"
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"}
                    }
                }
            },
            "/api/ws": {
                "get": {
                    "tags": ["Monitor"],
                    "summary": "WebSocket real-time events",
                    "description": "Upgrades to a WebSocket connection and pushes real-time JSON events (session creation, messages, workflow execution, task runs, channel messages) to all connected clients. Token extracted from `?token=` query parameter.",
                    "operationId": "websocketConnect",
                    "parameters": [
                        {
                            "name": "token",
                            "in": "query",
                            "required": false,
                            "schema": {"type": "string"},
                            "description": "Authentication token (required when auth_enabled is true)"
                        }
                    ],
                    "responses": {
                        "101": {
                            "description": "WebSocket upgrade successful"
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"}
                    }
                }
            },
            "/monitor": {
                "get": {
                    "tags": ["Monitor"],
                    "summary": "Monitor dashboard HTML",
                    "description": "Serves a self-contained HTML monitoring dashboard with auto-refresh. No authentication required.",
                    "operationId": "monitorDashboard",
                    "security": [],
                    "responses": {
                        "200": {
                            "description": "Monitor dashboard HTML page",
                            "content": {
                                "text/html": {}
                            }
                        }
                    }
                }
            },
            "/api/notifications/test-email": {
                "post": {
                    "tags": ["Notifications"],
                    "summary": "Send test email",
                    "description": "Sends a test email to the configured recipients to verify SMTP configuration. Returns 400 if the email notifier is not configured.",
                    "operationId": "testEmail",
                    "responses": {
                        "200": {
                            "description": "Test email sent successfully",
                            "content": {
                                "application/json": {
                                    "example": {
                                        "success": true,
                                        "message": "Test email sent. Check your inbox."
                                    }
                                }
                            }
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "500": {"$ref": "#/components/responses/InternalError"}
                    }
                }
            },
            "/p/{publish_id}": {
                "get": {
                    "tags": ["Publish"],
                    "summary": "Get published workflow result",
                    "description": "Serves a published workflow result as a styled HTML page with dark theme, TOC sidebar, search, syntax highlighting, and download button. Always returns HTTP 200 (shows a 404-style page if not found). No authentication required.",
                    "operationId": "getPublished",
                    "security": [],
                    "parameters": [
                        {"$ref": "#/components/parameters/PublishIdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Published result HTML page",
                            "content": {
                                "text/html": {}
                            }
                        }
                    }
                }
            },
            "/api/publish/{id}": {
                "delete": {
                    "tags": ["Publish"],
                    "summary": "Delete a published page",
                    "description": "Deletes a published workflow result page.",
                    "operationId": "deletePublished",
                    "parameters": [
                        {"$ref": "#/components/parameters/IdParam"}
                    ],
                    "responses": {
                        "200": {
                            "description": "Page deleted",
                            "content": {
                                "application/json": {
                                    "example": {
                                        "deleted": true,
                                        "id": "550e8400-..."
                                    }
                                }
                            }
                        },
                        "401": {"$ref": "#/components/responses/Unauthorized"}
                    }
                }
            }
        },
        "components": {
            "parameters": {
                "IdParam": {
                    "name": "id",
                    "in": "path",
                    "required": true,
                    "schema": {
                        "type": "string",
                        "format": "uuid"
                    },
                    "description": "Resource identifier (UUID)"
                },
                "KeyParam": {
                    "name": "key",
                    "in": "path",
                    "required": true,
                    "schema": {
                        "type": "string"
                    },
                    "description": "Configuration key name"
                },
                "PublishIdParam": {
                    "name": "publish_id",
                    "in": "path",
                    "required": true,
                    "schema": {
                        "type": "string"
                    },
                    "description": "Published result identifier"
                },
                "SearchQueryParam": {
                    "name": "q",
                    "in": "query",
                    "required": false,
                    "schema": {
                        "type": "string"
                    },
                    "description": "The search query string"
                },
                "SearchTypeParam": {
                    "name": "type",
                    "in": "query",
                    "required": false,
                    "schema": {
                        "type": "string",
                        "enum": ["sessions", "messages"]
                    },
                    "description": "Filter by result type"
                },
                "PageParam": {
                    "name": "page",
                    "in": "query",
                    "required": false,
                    "schema": {
                        "type": "integer",
                        "minimum": 1,
                        "default": 1
                    },
                    "description": "Page number (1-based)"
                },
                "LimitQuery": {
                    "name": "limit",
                    "in": "query",
                    "required": false,
                    "schema": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "default": 20
                    },
                    "description": "Results per page (max 100)"
                },
                "ExportFormatParam": {
                    "name": "format",
                    "in": "query",
                    "required": false,
                    "schema": {
                        "type": "string",
                        "enum": ["json", "markdown", "html"],
                        "default": "json"
                    },
                    "description": "Export format"
                },
                "MsgSignatureParam": {
                    "name": "msg_signature",
                    "in": "query",
                    "required": true,
                    "schema": {"type": "string"},
                    "description": "SHA1 signature for verification"
                },
                "TimestampParam": {
                    "name": "timestamp",
                    "in": "query",
                    "required": true,
                    "schema": {"type": "string"},
                    "description": "Unix timestamp"
                },
                "NonceParam": {
                    "name": "nonce",
                    "in": "query",
                    "required": true,
                    "schema": {"type": "string"},
                    "description": "Random nonce string"
                },
                "EchostrParam": {
                    "name": "echostr",
                    "in": "query",
                    "required": true,
                    "schema": {"type": "string"},
                    "description": "Encrypted verification string"
                }
            },
            "schemas": {
                "HealthResponse": {
                    "type": "object",
                    "properties": {
                        "status": {"type": "string", "example": "ok"},
                        "version": {"type": "string", "example": "0.1.0"},
                        "timestamp": {"type": "string", "format": "date-time"}
                    }
                },
                "InfoResponse": {
                    "type": "object",
                    "properties": {
                        "version": {"type": "string"},
                        "status": {"type": "string", "example": "running"},
                        "timestamp": {"type": "string", "format": "date-time"},
                        "stats": {
                            "type": "object",
                            "properties": {
                                "sessions": {"type": "integer"},
                                "active_channels": {"type": "integer"},
                                "workflows": {"type": "integer"},
                                "enabled_tasks": {"type": "integer"},
                                "tools": {"type": "integer"}
                            }
                        },
                        "tools": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "channels": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "features": {
                            "type": "object",
                            "properties": {
                                "chat": {"type": "boolean"},
                                "streaming": {"type": "boolean"},
                                "workflows": {"type": "boolean"},
                                "scheduler": {"type": "boolean"},
                                "publishing": {"type": "boolean"},
                                "onboarding": {"type": "boolean"}
                            }
                        }
                    }
                },
                "AuthStatusResponse": {
                    "type": "object",
                    "properties": {
                        "auth_enabled": {"type": "boolean", "description": "Whether token-based auth is active"},
                        "configured": {"type": "boolean", "description": "Whether an admin token has been set"}
                    }
                },
                "LoginRequest": {
                    "type": "object",
                    "required": ["token"],
                    "properties": {
                        "token": {"type": "string", "description": "The authentication token to validate"}
                    }
                },
                "LoginResponse": {
                    "type": "object",
                    "properties": {
                        "valid": {"type": "boolean"},
                        "token": {"type": "string"},
                        "message": {"type": "string"}
                    }
                },
                "ConfigMap": {
                    "type": "object",
                    "description": "Flat key-value map of configuration entries",
                    "additionalProperties": {"type": "string"}
                },
                "ConfigValueResponse": {
                    "type": "object",
                    "properties": {
                        "key": {"type": "string"},
                        "value": {"type": "string"}
                    }
                },
                "CreateSessionRequest": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": {"type": "string", "description": "Session display name"},
                        "agent_id": {"type": "string", "description": "Optional agent identifier"},
                        "system_prompt": {"type": "string", "description": "System prompt for the conversation"},
                        "model": {"type": "string", "default": "deepseek-chat", "description": "LLM model name"},
                        "temperature": {"type": "number", "format": "double", "default": 0.7, "minimum": 0.0, "maximum": 2.0},
                        "max_tokens": {"type": "integer", "default": 4096}
                    }
                },
                "UpdateSessionRequest": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "agent_id": {"type": "string"},
                        "system_prompt": {"type": "string"},
                        "model": {"type": "string"},
                        "temperature": {"type": "number", "format": "double", "minimum": 0.0, "maximum": 2.0},
                        "max_tokens": {"type": "integer"}
                    }
                },
                "SessionRow": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "format": "uuid"},
                        "name": {"type": "string"},
                        "agent_id": {"type": "string", "nullable": true},
                        "system_prompt": {"type": "string", "nullable": true},
                        "model": {"type": "string"},
                        "temperature": {"type": "number", "format": "double"},
                        "max_tokens": {"type": "integer"},
                        "channel": {"type": "string"},
                        "channel_chat_id": {"type": "string", "nullable": true},
                        "created_at": {"type": "string", "format": "date-time"},
                        "updated_at": {"type": "string", "format": "date-time"}
                    }
                },
                "MessageRow": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "format": "uuid"},
                        "session_id": {"type": "string", "format": "uuid"},
                        "role": {"type": "string", "enum": ["user", "assistant", "system", "tool"]},
                        "content": {"type": "string"},
                        "tool_calls": {"type": "string", "nullable": true, "description": "JSON string of tool call details"},
                        "tool_call_id": {"type": "string", "nullable": true},
                        "created_at": {"type": "string", "format": "date-time"}
                    }
                },
                "ChatRequest": {
                    "type": "object",
                    "required": ["message"],
                    "properties": {
                        "session_id": {"type": "string", "format": "uuid", "description": "Existing session ID. Omit to auto-create a new session."},
                        "message": {"type": "string", "description": "The user message content"},
                        "model": {"type": "string", "description": "Optional: override the session's default model"}
                    }
                },
                "ChatResponse": {
                    "type": "object",
                    "properties": {
                        "session_id": {"type": "string", "format": "uuid"},
                        "message": {"type": "string", "description": "The full AI response text"}
                    }
                },
                "SseEvent": {
                    "type": "object",
                    "description": "SSE event discriminated by `type` field",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["thinking", "delta", "tool_start", "tool_end", "done", "error"]
                        }
                    }
                },
                "CreateChannelRequest": {
                    "type": "object",
                    "required": ["channel_type", "name"],
                    "properties": {
                        "channel_type": {"type": "string", "enum": ["feishu", "wechat_work", "qq", "webhook"]},
                        "name": {"type": "string", "description": "Display name for this channel"},
                        "config": {"type": "object", "description": "Channel-specific JSON configuration"}
                    }
                },
                "UpdateChannelRequest": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "enabled": {"type": "boolean"},
                        "config": {"type": "object"}
                    }
                },
                "ChannelRow": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "format": "uuid"},
                        "channel_type": {"type": "string"},
                        "name": {"type": "string"},
                        "enabled": {"type": "boolean"},
                        "config": {"type": "string", "description": "JSON string of channel configuration"},
                        "created_at": {"type": "string", "format": "date-time"},
                        "updated_at": {"type": "string", "format": "date-time"}
                    }
                },
                "CreateWorkflowRequest": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": {"type": "string"},
                        "description": {"type": "string", "default": ""},
                        "definition": {"type": "object", "description": "DAG definition with steps and edges"},
                        "trigger_type": {"type": "string", "enum": ["manual", "cron"], "default": "manual"},
                        "cron_expression": {"type": "string", "description": "Cron expression (required when trigger_type is 'cron')"}
                    }
                },
                "UpdateWorkflowRequest": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "description": {"type": "string"},
                        "definition": {"type": "object", "description": "Replaces the entire DAG definition"},
                        "trigger_type": {"type": "string", "enum": ["manual", "cron"]},
                        "cron_expression": {"type": "string"},
                        "enabled": {"type": "boolean"}
                    }
                },
                "WorkflowRow": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "format": "uuid"},
                        "name": {"type": "string"},
                        "description": {"type": "string"},
                        "definition": {"type": "string", "description": "JSON string of the DAG definition"},
                        "trigger_type": {"type": "string"},
                        "cron_expression": {"type": "string", "nullable": true},
                        "enabled": {"type": "boolean"},
                        "last_run_at": {"type": "string", "nullable": true, "format": "date-time"},
                        "created_at": {"type": "string", "format": "date-time"},
                        "updated_at": {"type": "string", "format": "date-time"}
                    }
                },
                "WorkflowRunResult": {
                    "type": "object",
                    "description": "Result of executing a workflow DAG",
                    "properties": {
                        "status": {"type": "string", "enum": ["success", "error", "skipped", "pending", "running"]},
                        "steps": {"type": "array", "items": {"type": "object"}}
                    }
                },
                "WorkflowRunRow": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "format": "uuid"},
                        "workflow_id": {"type": "string", "format": "uuid"},
                        "status": {"type": "string"},
                        "started_at": {"type": "string", "format": "date-time"},
                        "finished_at": {"type": "string", "nullable": true, "format": "date-time"},
                        "result": {"type": "string", "nullable": true},
                        "publish_url": {"type": "string", "nullable": true}
                    }
                },
                "CreateTaskRequest": {
                    "type": "object",
                    "required": ["name", "cron_expression", "prompt"],
                    "properties": {
                        "name": {"type": "string", "description": "Task display name"},
                        "cron_expression": {"type": "string", "description": "5-field cron expression"},
                        "prompt": {"type": "string", "description": "LLM prompt to execute on each run"},
                        "session_id": {"type": "string", "format": "uuid", "description": "Optional session to use for context"},
                        "model": {"type": "string", "default": "deepseek-chat"},
                        "enabled": {"type": "boolean", "default": true}
                    }
                },
                "UpdateTaskRequest": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "cron_expression": {"type": "string"},
                        "prompt": {"type": "string"},
                        "session_id": {"type": "string", "format": "uuid"},
                        "model": {"type": "string"},
                        "enabled": {"type": "boolean"}
                    }
                },
                "ScheduledTaskRow": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "format": "uuid"},
                        "name": {"type": "string"},
                        "cron_expression": {"type": "string"},
                        "prompt": {"type": "string"},
                        "session_id": {"type": "string", "nullable": true, "format": "uuid"},
                        "model": {"type": "string"},
                        "enabled": {"type": "boolean"},
                        "created_at": {"type": "string", "format": "date-time"},
                        "updated_at": {"type": "string", "format": "date-time"}
                    }
                },
                "TaskRunResponse": {
                    "type": "object",
                    "properties": {
                        "task_id": {"type": "string", "format": "uuid"},
                        "result": {"type": "string", "description": "LLM response text"}
                    }
                },
                "TaskLogRow": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "format": "uuid"},
                        "task_id": {"type": "string", "format": "uuid"},
                        "status": {"type": "string"},
                        "output": {"type": "string", "nullable": true},
                        "error": {"type": "string", "nullable": true},
                        "started_at": {"type": "string", "format": "date-time"},
                        "finished_at": {"type": "string", "nullable": true, "format": "date-time"}
                    }
                },
                "SearchResponse": {
                    "type": "object",
                    "properties": {
                        "total": {"type": "integer", "description": "Total number of matching results"},
                        "page": {"type": "integer", "description": "Current page number"},
                        "results": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "type": {"type": "string", "enum": ["session", "message"]},
                                    "session_id": {"type": "string", "format": "uuid"},
                                    "session_name": {"type": "string"},
                                    "snippet": {"type": "string", "description": "Text snippet with match context"},
                                    "score": {"type": "integer", "minimum": 0, "maximum": 100},
                                    "message_id": {"type": "string", "format": "uuid"}
                                }
                            }
                        }
                    }
                },
                "BulkExportRequest": {
                    "type": "object",
                    "required": ["session_ids"],
                    "properties": {
                        "session_ids": {
                            "type": "array",
                            "items": {"type": "string", "format": "uuid"},
                            "maxItems": 50,
                            "description": "List of session IDs to export (max 50)"
                        },
                        "format": {"type": "string", "enum": ["json", "markdown", "html"], "default": "json"}
                    }
                },
                "BackupEntry": {
                    "type": "object",
                    "properties": {
                        "filename": {"type": "string"},
                        "size_bytes": {"type": "integer", "format": "int64"},
                        "modified": {"type": "string", "nullable": true, "format": "date-time"}
                    }
                },
                "MonitorStats": {
                    "type": "object",
                    "properties": {
                        "server": {
                            "type": "object",
                            "properties": {
                                "uptime_secs": {"type": "integer"},
                                "uptime_display": {"type": "string"},
                                "request_count": {"type": "integer"},
                                "active_sse_connections": {"type": "integer"},
                                "active_ws_connections": {"type": "integer"}
                            }
                        },
                        "data": {
                            "type": "object",
                            "properties": {
                                "total_sessions": {"type": "integer"},
                                "total_messages": {"type": "integer"},
                                "db_size_bytes": {"type": "integer"},
                                "db_size_display": {"type": "string"},
                                "memory_rss_bytes": {"type": "integer", "nullable": true},
                                "memory_rss_display": {"type": "string"}
                            }
                        },
                        "llm": {
                            "type": "object",
                            "properties": {
                                "calls_total": {"type": "integer"},
                                "calls_success": {"type": "integer"},
                                "calls_error": {"type": "integer"}
                            }
                        },
                        "channels": {
                            "type": "object",
                            "description": "Per-channel message counts",
                            "additionalProperties": {"type": "integer"}
                        },
                        "recent_errors": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "timestamp": {"type": "string", "format": "date-time"},
                                    "message": {"type": "string"}
                                }
                            }
                        }
                    }
                },
                "ErrorResponse": {
                    "type": "object",
                    "properties": {
                        "error": {"type": "string", "description": "Human-readable error message"}
                    }
                },
                "RateLimitError": {
                    "type": "object",
                    "properties": {
                        "error": {"type": "string"},
                        "retry_after": {"type": "integer", "description": "Seconds until the client can retry"}
                    }
                }
            },
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "UUID",
                    "description": "Admin token authentication. The token is set on first startup and stored in the database under the `admin_token` config key."
                }
            },
            "responses": {
                "BadRequest": {
                    "description": "Invalid request parameters or body",
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/ErrorResponse"},
                            "example": {"error": "Missing required field"}
                        }
                    }
                },
                "Unauthorized": {
                    "description": "Missing or invalid authentication token",
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/ErrorResponse"},
                            "example": {"error": "Unauthorized — invalid or missing authentication token"}
                        }
                    }
                },
                "NotFound": {
                    "description": "Requested resource not found",
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/ErrorResponse"},
                            "example": {"error": "Session 'abc-123' not found"}
                        }
                    }
                },
                "RateLimited": {
                    "description": "Too many requests — rate limit exceeded",
                    "headers": {
                        "Retry-After": {
                            "schema": {"type": "integer"},
                            "description": "Seconds to wait before retrying"
                        },
                        "X-RateLimit-Limit": {
                            "schema": {"type": "integer"},
                            "description": "Request limit per window"
                        },
                        "X-RateLimit-Remaining": {
                            "schema": {"type": "integer"},
                            "description": "Remaining requests in the current window"
                        },
                        "X-RateLimit-Reset": {
                            "schema": {"type": "integer"},
                            "description": "Seconds until the window resets"
                        }
                    },
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/RateLimitError"},
                            "example": {
                                "error": "Rate limited",
                                "retry_after": 42
                            }
                        }
                    }
                },
                "InternalError": {
                    "description": "Internal server error",
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/ErrorResponse"},
                            "example": {"error": "Internal error"}
                        }
                    }
                }
            }
        },
        "security": [
            {"bearerAuth": []}
        ]
    })
}

// ── Route handlers ──────────────────────────────────────────────────────

/// `GET /api/openapi.json` -- returns the full OpenAPI 3.0 specification.
///
/// The spec is built once at startup via `LazyLock` and served
/// without cloning from every request.
pub async fn openapi_json() -> Json<&'static Value> {
    Json(&OPENAPI_SPEC)
}

/// `GET /api/docs` -- serves the Swagger UI HTML page with dark theme.
///
/// Loads the Swagger UI bundle from CDN and points it at `/api/openapi.json`.
pub async fn api_docs() -> Html<&'static str> {
    Html(SWAGGER_UI_HTML)
}

/// Self-contained Swagger UI page with dark theme matching the app.
const SWAGGER_UI_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>AI Agent API — Documentation</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui.css">
  <style>
    /* ═══════════════════════════════════════════
       Dark theme matching AI Agent app design
       ═══════════════════════════════════════════ */
    :root {
      --bg-primary: #0a0a0f;
      --bg-secondary: #111118;
      --bg-tertiary: #1a1a24;
      --bg-elevated: #22222f;
      --text-primary: #f0f0f5;
      --text-secondary: #a0a0b8;
      --text-tertiary: #606078;
      --border-color: rgba(255,255,255,0.06);
      --accent-primary: #8b5cf6;
      --accent-secondary: #3b82f6;
    }
    html { box-sizing: border-box; overflow: -moz-scrollbars-vertical; overflow-y: scroll; }
    *, *:before, *:after { box-sizing: inherit; }
    body {
      margin: 0;
      background: var(--bg-primary);
    }

    /* ── Top bar ── */
    .topbar {
      background: var(--bg-secondary);
      border-bottom: 1px solid var(--border-color);
      padding: 0 0;
      display: flex;
      align-items: center;
      height: 56px;
    }
    .topbar-wrapper {
      max-width: 1460px;
      width: 100%;
      margin: 0 auto;
      display: flex;
      align-items: center;
      padding: 0 24px;
    }
    .topbar a {
      display: flex;
      align-items: center;
      gap: 10px;
      text-decoration: none;
    }
    .topbar .title {
      font-size: 1.15rem;
      font-weight: 700;
      background: linear-gradient(135deg, #8b5cf6, #3b82f6);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
      background-clip: text;
      font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    }
    .version-tag {
      font-size: 0.7rem;
      font-weight: 500;
      background: var(--bg-elevated);
      color: var(--text-secondary);
      padding: 2px 10px;
      border-radius: 12px;
      border: 1px solid var(--border-color);
      font-family: 'JetBrains Mono', 'Fira Code', monospace;
    }

    /* ── Swagger UI overrides ── */
    .swagger-ui {
      font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
      color: var(--text-primary);
    }
    .swagger-ui .topbar { display: none; }

    /* Info section */
    .swagger-ui .info {
      margin: 30px 0;
    }
    .swagger-ui .info .title {
      font-family: 'Inter', sans-serif;
      font-size: 2rem;
      font-weight: 700;
      color: var(--text-primary);
    }
    .swagger-ui .info .description p {
      color: var(--text-secondary);
      font-size: 0.95rem;
      line-height: 1.7;
    }
    .swagger-ui .info a {
      color: var(--accent-primary);
    }
    .swagger-ui .info .base-url {
      color: var(--text-tertiary);
      font-family: 'JetBrains Mono', 'Fira Code', monospace;
      font-size: 0.85rem;
    }

    /* Schemes */
    .swagger-ui .scheme-container {
      background: var(--bg-secondary);
      border: 1px solid var(--border-color);
      border-radius: 12px;
      padding: 20px 24px;
      margin: 0 0 28px;
      box-shadow: none;
    }
    .swagger-ui .scheme-container .schemes {
      display: flex;
      align-items: center;
      gap: 12px;
    }
    .swagger-ui .scheme-container .schemes>label {
      color: var(--text-secondary);
      font-size: 0.9rem;
      font-weight: 500;
    }
    .swagger-ui .auth-wrapper .authorize {
      background: var(--bg-elevated);
      color: var(--text-primary);
      border: 1px solid var(--border-color);
      border-radius: 8px;
      padding: 8px 20px;
      font-weight: 600;
      font-size: 0.85rem;
      cursor: pointer;
      transition: all 150ms ease;
    }
    .swagger-ui .auth-wrapper .authorize:hover {
      border-color: var(--accent-primary);
      background: var(--bg-tertiary);
    }
    .swagger-ui .auth-wrapper .authorize svg { fill: var(--accent-primary); }

    /* Tags */
    .swagger-ui .opblock-tag {
      font-family: 'Inter', sans-serif;
      font-size: 1.1rem;
      font-weight: 600;
      color: var(--text-primary);
      border-bottom: 1px solid var(--border-color);
      padding: 12px 0;
    }
    .swagger-ui .opblock-tag:hover { background: var(--bg-glass, rgba(255,255,255,0.02)); }
    .swagger-ui .opblock-tag svg { fill: var(--text-secondary); }

    /* Operation blocks */
    .swagger-ui .opblock {
      border-radius: 10px;
      border: 1px solid var(--border-color);
      margin: 0 0 12px;
      box-shadow: none;
    }
    .swagger-ui .opblock .opblock-summary {
      border: none;
      padding: 10px 16px;
    }
    .swagger-ui .opblock .opblock-summary-description {
      color: var(--text-secondary);
      font-size: 0.85rem;
    }
    .swagger-ui .opblock .opblock-summary-path {
      font-family: 'JetBrains Mono', 'Fira Code', monospace;
      font-size: 0.85rem;
      font-weight: 500;
    }

    /* GET method */
    .swagger-ui .opblock.opblock-get {
      background: rgba(59,130,246,0.04);
      border-color: rgba(59,130,246,0.15);
    }
    .swagger-ui .opblock.opblock-get .opblock-summary-method {
      background: #3b82f6;
      font-weight: 700;
      font-size: 0.75rem;
      padding: 4px 12px;
      border-radius: 6px;
    }
    /* POST method */
    .swagger-ui .opblock.opblock-post {
      background: rgba(34,197,94,0.04);
      border-color: rgba(34,197,94,0.15);
    }
    .swagger-ui .opblock.opblock-post .opblock-summary-method {
      background: #22c55e;
      font-weight: 700;
      font-size: 0.75rem;
      padding: 4px 12px;
      border-radius: 6px;
    }
    /* PUT method */
    .swagger-ui .opblock.opblock-put {
      background: rgba(245,158,11,0.04);
      border-color: rgba(245,158,11,0.15);
    }
    .swagger-ui .opblock.opblock-put .opblock-summary-method {
      background: #f59e0b;
      font-weight: 700;
      font-size: 0.75rem;
      padding: 4px 12px;
      border-radius: 6px;
    }
    /* DELETE method */
    .swagger-ui .opblock.opblock-delete {
      background: rgba(239,68,68,0.04);
      border-color: rgba(239,68,68,0.15);
    }
    .swagger-ui .opblock.opblock-delete .opblock-summary-method {
      background: #ef4444;
      font-weight: 700;
      font-size: 0.75rem;
      padding: 4px 12px;
      border-radius: 6px;
    }

    /* Opblock body */
    .swagger-ui .opblock-body {
      background: transparent;
    }
    .swagger-ui .opblock-body pre {
      background: var(--bg-primary);
      border: 1px solid var(--border-color);
      border-radius: 8px;
      font-family: 'JetBrains Mono', 'Fira Code', monospace;
      font-size: 0.82rem;
    }
    .swagger-ui .opblock-body pre.microlight {
      background: var(--bg-primary);
    }

    /* Parameter table */
    .swagger-ui .parameters-col_description,
    .swagger-ui .parameter__name,
    .swagger-ui .parameter__type {
      color: var(--text-secondary);
      font-size: 0.85rem;
    }
    .swagger-ui table thead tr td,
    .swagger-ui table thead tr th {
      color: var(--text-tertiary);
      font-size: 0.8rem;
      text-transform: uppercase;
      border-bottom: 1px solid var(--border-color);
    }
    .swagger-ui .parameter__name.required:after {
      color: #ef4444;
    }
    .swagger-ui .parameter__name.required span {
      color: #ef4444;
    }

    /* Response table */
    .swagger-ui .responses-inner {
      background: var(--bg-secondary);
      border-radius: 8px;
    }
    .swagger-ui .responses-inner h4,
    .swagger-ui .responses-inner h5 {
      color: var(--text-primary);
    }
    .swagger-ui .response-col_status {
      color: var(--text-secondary);
    }
    .swagger-ui .response-col_description p {
      color: var(--text-tertiary);
      font-size: 0.85rem;
    }

    /* Model / Schema */
    .swagger-ui .model {
      color: var(--text-secondary);
      font-size: 0.85rem;
    }
    .swagger-ui .model-box {
      background: var(--bg-secondary);
      border: 1px solid var(--border-color);
      border-radius: 8px;
    }
    .swagger-ui .model .property {
      color: var(--text-secondary);
    }
    .swagger-ui .model .property .prop-type {
      color: var(--accent-primary);
    }
    .swagger-ui .model-toggle {
      font-size: 0.85rem;
    }
    .swagger-ui section.models {
      border: 1px solid var(--border-color);
      border-radius: 12px;
      background: var(--bg-secondary);
    }
    .swagger-ui section.models h4 {
      color: var(--text-primary);
    }
    .swagger-ui .model-title {
      color: var(--text-primary);
      font-size: 0.9rem;
    }

    /* Buttons */
    .swagger-ui .btn {
      background: var(--bg-elevated);
      color: var(--text-primary);
      border: 1px solid var(--border-color);
      border-radius: 8px;
      font-weight: 600;
      font-size: 0.82rem;
      transition: all 150ms ease;
    }
    .swagger-ui .btn:hover {
      border-color: var(--accent-primary);
      background: var(--bg-tertiary);
    }
    .swagger-ui .btn.try-out__btn {
      background: var(--accent-primary);
      color: #fff;
      border-color: var(--accent-primary);
    }
    .swagger-ui .btn.try-out__btn:hover {
      opacity: 0.9;
    }
    .swagger-ui .btn.cancel {
      background: var(--bg-elevated);
      color: var(--text-primary);
      border-color: var(--border-color);
    }

    /* Try it out */
    .swagger-ui .execute-wrapper .btn {
      background: var(--accent-primary);
      color: #fff;
      border: none;
      border-radius: 8px;
      font-weight: 600;
    }
    .swagger-ui textarea,
    .swagger-ui input[type=text] {
      background: var(--bg-primary);
      border: 1px solid var(--border-color);
      border-radius: 6px;
      color: var(--text-primary);
      font-family: 'JetBrains Mono', 'Fira Code', monospace;
      font-size: 0.82rem;
    }
    .swagger-ui textarea:focus,
    .swagger-ui input[type=text]:focus {
      border-color: var(--accent-primary);
      outline: none;
      box-shadow: 0 0 0 2px rgba(139,92,246,0.15);
    }
    .swagger-ui select {
      background: var(--bg-primary);
      border: 1px solid var(--border-color);
      border-radius: 6px;
      color: var(--text-primary);
    }

    /* Live response */
    .swagger-ui .live-responses-table {
      background: var(--bg-secondary);
      border-radius: 8px;
    }
    .swagger-ui .responses-table .response-col_description pre {
      background: var(--bg-primary);
      border: 1px solid var(--border-color);
      border-radius: 8px;
    }

    /* Authorize dialog */
    .swagger-ui .dialog-ux .modal-ux {
      background: var(--bg-secondary);
      border: 1px solid var(--border-color);
      border-radius: 12px;
    }
    .swagger-ui .dialog-ux .modal-ux-header h3 {
      color: var(--text-primary);
    }
    .swagger-ui .dialog-ux .modal-ux-content {
      color: var(--text-secondary);
    }
    .swagger-ui .dialog-ux .modal-ux-content label {
      color: var(--text-secondary);
      font-size: 0.85rem;
    }
    .swagger-ui .auth-btn-wrapper .btn-done {
      background: var(--accent-primary);
      color: #fff;
      border: none;
      border-radius: 8px;
      font-weight: 600;
    }
    .swagger-ui .dialog-ux .modal-ux-header .close-modal {
      background: none;
      border: none;
      color: var(--text-tertiary);
      font-size: 1.2rem;
      cursor: pointer;
    }

    /* Errors */
    .swagger-ui .errors-wrapper {
      background: rgba(239,68,68,0.08);
      border: 1px solid rgba(239,68,68,0.2);
      border-radius: 8px;
    }
    .swagger-ui .errors-wrapper .errors h4 {
      color: #ef4444;
    }

    /* Loading */
    .swagger-ui .loading-container .loading {
      color: var(--text-tertiary);
    }

    /* Highlight markdown */
    .swagger-ui .markdown p,
    .swagger-ui .markdown li,
    .swagger-ui .renderedMarkdown p,
    .swagger-ui .renderedMarkdown li {
      color: var(--text-secondary);
      font-family: 'Inter', sans-serif;
      line-height: 1.6;
    }
    .swagger-ui .markdown code,
    .swagger-ui .renderedMarkdown code {
      background: var(--bg-primary);
      border: 1px solid var(--border-color);
      border-radius: 4px;
      padding: 2px 6px;
      font-family: 'JetBrains Mono', 'Fira Code', monospace;
      color: var(--accent-primary);
    }
    .swagger-ui .markdown pre code {
      background: var(--bg-primary);
      border: 1px solid var(--border-color);
      border-radius: 8px;
      font-size: 0.82rem;
      padding: 12px 16px;
    }
    .swagger-ui .prop-type {
      color: var(--accent-primary);
    }

    /* Scrollbar */
    ::-webkit-scrollbar { width: 6px; height: 6px; }
    ::-webkit-scrollbar-track { background: transparent; }
    ::-webkit-scrollbar-thumb { background: var(--border-color); border-radius: 3px; }
    ::-webkit-scrollbar-thumb:hover { background: rgba(255,255,255,0.12); }

    /* Authorize button SVG */
    .swagger-ui .auth-wrapper .authorize svg {
      width: 18px;
      height: 18px;
      margin-right: 6px;
    }
  </style>
</head>
<body>
  <div class="topbar">
    <div class="topbar-wrapper">
      <a href="/">
        <span class="title">AI Agent API</span>
        <span class="version-tag">vVERSION</span>
      </a>
    </div>
  </div>
  <div id="swagger-ui"></div>

  <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    window.onload = function () {
      window.ui = SwaggerUIBundle({
        url: '/api/openapi.json',
        dom_id: '#swagger-ui',
        deepLinking: true,
        presets: [SwaggerUIBundle.presets.apis],
        plugins: [SwaggerUIBundle.plugins.DownloadUrl],
        layout: 'BaseLayout',
        defaultModelsExpandDepth: 1,
        defaultModelExpandDepth: 2,
        defaultModelRendering: 'model',
        displayRequestDuration: true,
        docExpansion: 'list',
        filter: true,
        showExtensions: true,
        showCommonExtensions: true,
        tryItOutEnabled: true,
        syntaxHighlight: {
          activated: true,
          theme: 'monokai'
        }
      });
    };
  </script>
</body>
</html>"#;
