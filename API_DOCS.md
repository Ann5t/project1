# AI Agent REST API Reference

完整 REST API 参考文档，包含所有端点、请求/响应 Schema、curl 示例和错误码。

---

## 目录

- [通用约定](#通用约定)
- [认证说明](#认证说明)
- [错误码参考](#错误码参考)
- [1. Health](#1-health)
- [2. Config](#2-config)
- [3. Sessions](#3-sessions)
- [4. Chat](#4-chat)
- [5. Channels](#5-channels)
- [6. Workflows](#6-workflows)
- [7. Tasks](#7-tasks)
- [8. Monitor](#8-monitor)
- [9. Publish](#9-publish)

---

## 通用约定

| 项目 | 说明 |
|------|------|
| **Base URL** | `http://localhost:3000/api` |
| **Content-Type** | `application/json` |
| **字符编码** | UTF-8 |
| **日期格式** | ISO 8601 / RFC 3339 (如 `2026-06-19T10:30:00+08:00`) |

所有时间戳均为 UTC。

### 响应格式

**成功响应** (2xx): 直接返回 JSON 资源对象或 `{"status":"ok", ...}`。

**错误响应** (4xx/5xx):
```json
{
  "error": "人类可读的错误信息"
}
```

---

## 认证说明

当前版本 **不要求客户端认证**。

飞书回调端点 (`/api/channels/feishu/callback`) 使用飞书自身的 URL 验证（Challenge Echo）和可选的 AES-256-CBC 事件加密来保证安全性。

生产环境建议：
- 将服务器置于 VPN 或内网中
- 使用反向代理 (Nginx/Caddy) 添加 HTTP Basic Auth 或 IP 白名单
- 使用 HTTPS 加密所有通信

---

## 错误码参考

| HTTP 状态码 | 含义 | 触发条件 |
|-------------|------|----------|
| `200 OK` | 成功 | 正常响应 |
| `400 Bad Request` | 请求参数错误 | 缺少必填字段、JSON 格式错误、无效的配置值 |
| `404 Not Found` | 资源不存在 | 会话/工作流/任务/配置键未找到 |
| `429 Too Many Requests` | 频率限制 | DeepSeek API 返回 429（飞书频道有独立限流 30条/分钟/群） |
| `500 Internal Server Error` | 服务器内部错误 | 数据库错误、LLM API 错误、未预期的异常 |

### 错误响应体 Schema

```typescript
{
  error: string;  // 人类可读错误描述
}
```

---

## 1. Health

### `GET /api/health`

健康检查端点，无需数据库连接即可响应。

**响应** `200 OK`:
```json
{
  "status": "ok",
  "version": "0.1.0",
  "timestamp": "2026-06-19T10:30:00+00:00"
}
```

**curl 示例**:
```bash
curl http://localhost:3000/api/health
```

---

### `GET /api/info`

系统信息端点，返回运行状态、统计数据和特性列表。

**响应** `200 OK`:
```json
{
  "version": "0.1.0",
  "status": "running",
  "timestamp": "2026-06-19T10:30:00+00:00",
  "stats": {
    "sessions": 5,
    "active_channels": 2,
    "workflows": 3,
    "enabled_tasks": 1,
    "tools": 5
  },
  "tools": ["calculator", "get_current_time", "web_search", "read_file", "execute_shell"],
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
```

**curl 示例**:
```bash
curl http://localhost:3000/api/info
```

---

## 2. Config

### `GET /api/config`

获取所有配置项，以 key-value 映射返回。

**响应** `200 OK`:
```json
{
  "api_key": "sk-xxxxxxxxxxxxx",
  "base_url": "https://api.deepseek.com/v1",
  "default_model": "deepseek-chat",
  "flash_model": "deepseek-chat",
  "pro_model": "deepseek-reasoner",
  "system_prompt": "You are a helpful AI assistant.",
  "temperature": "0.7",
  "max_tokens": "4096",
  "max_tool_iterations": "10",
  "onboarding_completed": "true",
  "onboarding_step": "0",
  "theme": "dark",
  "language": "zh",
  "public_url": "https://ai.example.com"
}
```

**curl 示例**:
```bash
curl http://localhost:3000/api/config
```

---

### `PUT /api/config`

批量更新配置。请求体为 key-value 映射，所有非空的值会逐个写入。

**请求体**:
```json
{
  "api_key": "sk-new-key",
  "default_model": "deepseek-chat",
  "temperature": "0.8"
}
```

**响应** `200 OK`:
```json
{
  "updated": true
}
```

**curl 示例**:
```bash
curl -X PUT http://localhost:3000/api/config \
  -H "Content-Type: application/json" \
  -d '{"api_key":"sk-new-key","temperature":"0.8"}'
```

---

### `GET /api/config/{key}`

获取单个配置值。

**路径参数**:

| 参数 | 类型 | 说明 |
|------|------|------|
| `key` | string | 配置键名 (URL 编码) |

**响应** `200 OK`:
```json
{
  "key": "api_key",
  "value": "sk-xxxxxxxxxxxxx"
}
```

**错误** `404 Not Found`:
```json
{
  "error": "Config key 'unknown_key' not found"
}
```

**curl 示例**:
```bash
curl http://localhost:3000/api/config/default_model
```

---

### `PUT /api/config/{key}`

设置单个配置值。

**路径参数**:

| 参数 | 类型 | 说明 |
|------|------|------|
| `key` | string | 配置键名 |

**请求体**:
```json
{
  "value": "deepseek-reasoner"
}
```

**响应** `200 OK`:
```json
{
  "updated": true
}
```

**错误** `400 Bad Request`:
```json
{
  "error": "Missing 'value' field"
}
```

**curl 示例**:
```bash
curl -X PUT http://localhost:3000/api/config/default_model \
  -H "Content-Type: application/json" \
  -d '{"value":"deepseek-reasoner"}'
```

---

## 3. Sessions

### `GET /api/sessions`

列出所有会话，按更新时间倒序排列。

**响应** `200 OK`:
```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "My Chat Session",
    "agent_id": null,
    "system_prompt": null,
    "model": "deepseek-chat",
    "temperature": 0.7,
    "max_tokens": 4096,
    "channel": "web",
    "channel_chat_id": null,
    "created_at": "2026-06-19T08:00:00+00:00",
    "updated_at": "2026-06-19T10:25:00+00:00"
  }
]
```

**curl 示例**:
```bash
curl http://localhost:3000/api/sessions
```

---

### `POST /api/sessions`

创建新的会话。

**请求体**:

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `name` | string | 是 | - | 会话名称 |
| `agent_id` | string | 否 | null | 关联的 Agent ID |
| `system_prompt` | string | 否 | null | 系统提示词 |
| `model` | string | 否 | `"deepseek-chat"` | LLM 模型名 |
| `temperature` | number | 否 | `0.7` | 温度参数 (0.0-2.0) |
| `max_tokens` | integer | 否 | `4096` | 最大输出 token 数 |

**请求示例**:
```json
{
  "name": "Coding Assistant",
  "system_prompt": "You are a helpful Rust programming assistant.",
  "model": "deepseek-chat",
  "temperature": 0.3,
  "max_tokens": 8192
}
```

**响应** `200 OK`:
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "name": "Coding Assistant",
  "agent_id": null,
  "system_prompt": "You are a helpful Rust programming assistant.",
  "model": "deepseek-chat",
  "temperature": 0.3,
  "max_tokens": 8192,
  "channel": "web",
  "channel_chat_id": null,
  "created_at": "2026-06-19T10:30:00+00:00",
  "updated_at": "2026-06-19T10:30:00+00:00"
}
```

**curl 示例**:
```bash
curl -X POST http://localhost:3000/api/sessions \
  -H "Content-Type: application/json" \
  -d '{"name":"Coding Assistant","system_prompt":"You are a helpful Rust programming assistant.","temperature":0.3}'
```

---

### `GET /api/sessions/{id}`

获取单个会话详情。

**路径参数**:

| 参数 | 类型 | 说明 |
|------|------|------|
| `id` | string (UUID) | 会话 ID |

**响应** `200 OK`: 参见创建响应格式

**错误** `404 Not Found`:
```json
{
  "error": "Session 'xxx' not found"
}
```

**curl 示例**:
```bash
curl http://localhost:3000/api/sessions/550e8400-e29b-41d4-a716-446655440001
```

---

### `PUT /api/sessions/{id}`

更新会话。所有字段均为可选，未提供的字段保持不变。

**请求体**:

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | 否 | 新的会话名称 |
| `agent_id` | string | 否 | 关联的 Agent ID |
| `system_prompt` | string | 否 | 系统提示词 |
| `model` | string | 否 | LLM 模型名 |
| `temperature` | number | 否 | 温度参数 |
| `max_tokens` | integer | 否 | 最大 token 数 |

**请求示例**:
```json
{
  "name": "Renamed Session",
  "temperature": 0.5
}
```

**响应** `200 OK`: 更新后的完整会话对象

**curl 示例**:
```bash
curl -X PUT http://localhost:3000/api/sessions/550e8400-e29b-41d4-a716-446655440001 \
  -H "Content-Type: application/json" \
  -d '{"name":"Renamed Session","temperature":0.5}'
```

---

### `DELETE /api/sessions/{id}`

删除会话。该会话下的所有消息会级联删除。

**响应** `200 OK`:
```json
{
  "deleted": true
}
```

**curl 示例**:
```bash
curl -X DELETE http://localhost:3000/api/sessions/550e8400-e29b-41d4-a716-446655440001
```

---

### `GET /api/sessions/{id}/messages`

获取会话中所有消息，按时间顺序排列。

**响应** `200 OK`:
```json
[
  {
    "id": "msg-uuid-1",
    "session_id": "550e8400-...",
    "role": "user",
    "content": "What is Rust?",
    "tool_calls": null,
    "tool_call_id": null,
    "created_at": "2026-06-19T10:00:00+00:00"
  },
  {
    "id": "msg-uuid-2",
    "session_id": "550e8400-...",
    "role": "assistant",
    "content": "Rust is a systems programming language...",
    "tool_calls": null,
    "tool_call_id": null,
    "created_at": "2026-06-19T10:00:05+00:00"
  }
]
```

**消息角色** (`role`): `system`, `user`, `assistant`, `tool`

**curl 示例**:
```bash
curl http://localhost:3000/api/sessions/550e8400-e29b-41d4-a716-446655440001/messages
```

---

## 4. Chat

### `POST /api/chat`

发送消息并获取 AI 回复（非流式）。如果不提供 `session_id`，系统会自动创建新会话。

**请求体**:

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `session_id` | string (UUID) | 否 | 会话 ID，不提供则自动创建 |
| `message` | string | 是 | 用户消息内容 |
| `model` | string | 否 | 覆盖会话的默认模型 |

**请求示例**:
```json
{
  "session_id": "550e8400-e29b-41d4-a716-446655440001",
  "message": "Explain async/await in Rust"
}
```

**响应** `200 OK`:
```json
{
  "session_id": "550e8400-e29b-41d4-a716-446655440001",
  "message": "In Rust, async/await is a way to write asynchronous code..."
}
```

**curl 示例**:
```bash
# 使用已有会话
curl -X POST http://localhost:3000/api/chat \
  -H "Content-Type: application/json" \
  -d '{"session_id":"550e8400-e29b-41d4-a716-446655440001","message":"Explain async/await in Rust"}'

# 自动创建新会话
curl -X POST http://localhost:3000/api/chat \
  -H "Content-Type: application/json" \
  -d '{"message":"Hello, who are you?"}'
```

---

### `POST /api/chat/stream`

发送消息并通过 SSE (Server-Sent Events) 实时接收 AI 回复流。

**请求体**: 与 `/api/chat` 相同。

**响应** `200 OK` (SSE 流):

客户端收到的是 `text/event-stream` 格式的流式数据。每个事件携带 JSON 数据。

**SSE 事件类型**:

| `type` | 触发时机 | JSON 字段 |
|--------|---------|-----------|
| `thinking` | Agent 开始处理 | `{"type":"thinking","content":"Agent is thinking..."}` |
| `delta` | 响应文本片段 | `{"type":"delta","content":"In Rust"}` |
| `tool_start` | 工具调用开始 | `{"type":"tool_start","tool":"calculator","args":"{\"expression\":\"2+2\"}"}` |
| `tool_end` | 工具调用完成 | `{"type":"tool_end","tool":"calculator","result":"Result: 4"}` |
| `done` | 流结束 | `{"type":"done","session_id":"550e8400-..."}` |
| `error` | 发生错误 | `{"type":"error","message":"LLM API error: ..."}` |

**curl 示例**:
```bash
curl -N -X POST http://localhost:3000/api/chat/stream \
  -H "Content-Type: application/json" \
  -d '{"session_id":"550e8400-e29b-41d4-a716-446655440001","message":"Calculate 2+2 and tell me the result"}'
```

**JavaScript (EventSource) 示例**:
```javascript
// 注意: EventSource 不支持 POST，需要自定义 fetch 实现
async function streamChat(sessionId, message) {
  const response = await fetch('/api/chat/stream', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ session_id: sessionId, message }),
  });

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() || '';

    for (const line of lines) {
      if (line.startsWith('data: ')) {
        const data = JSON.parse(line.slice(6));
        if (data.type === 'delta') {
          console.log('Delta:', data.content);
        } else if (data.type === 'done') {
          console.log('Stream complete');
        } else if (data.type === 'error') {
          console.error('Error:', data.message);
        }
      }
    }
  }
}
```

---

## 5. Channels

### `GET /api/channels`

列出所有渠道配置。

**响应** `200 OK`:
```json
[
  {
    "id": "ch-uuid-1",
    "channel_type": "feishu",
    "name": "飞书 Bot",
    "enabled": true,
    "config": "{\"app_id\":\"cli_xxx\",\"app_secret\":\"...\"}",
    "created_at": "2026-06-19T08:00:00+00:00",
    "updated_at": "2026-06-19T10:00:00+00:00"
  }
]
```

**curl 示例**:
```bash
curl http://localhost:3000/api/channels
```

---

### `POST /api/channels`

创建新渠道。

**请求体**:

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `channel_type` | string | 是 | 渠道类型: `feishu`, `qq`, `wechat_work`, `webhook` |
| `name` | string | 是 | 渠道显示名称 |
| `config` | object | 否 | 渠道配置 JSON (见各渠道配置格式) |

**请求示例 (飞书)**:
```json
{
  "channel_type": "feishu",
  "name": "我的飞书 Bot",
  "config": {
    "app_id": "cli_xxxxxxxxxxxxx",
    "app_secret": "xxxxxxxxxxxxxxxxxxxxxx",
    "verification_token": "xxxxxxxxxxxxxxxxxxxxxx",
    "encrypt_key": ""
  }
}
```

**请求示例 (QQ)**:
```json
{
  "channel_type": "qq",
  "name": "我的 QQ Bot",
  "config": {
    "app_id": "10xxxxxxxx",
    "client_secret": "xxxxxxxxxxxx",
    "bot_secret": "xxxxxxxxxxxx"
  }
}
```

**请求示例 (企业微信)**:
```json
{
  "channel_type": "wechat_work",
  "name": "企业微信 Bot",
  "config": {
    "corp_id": "wwxxxxxxxxxxxx",
    "corp_secret": "xxxxxxxxxxxxxxxxxxxxxx",
    "agent_id": "1000001",
    "token": "xxxxxxxxxx",
    "encoding_aes_key": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  }
}
```

**响应** `200 OK`: 创建的完整渠道对象（含自动生成的 UUID）

**curl 示例**:
```bash
curl -X POST http://localhost:3000/api/channels \
  -H "Content-Type: application/json" \
  -d '{"channel_type":"feishu","name":"My Feishu Bot","config":{"app_id":"cli_xxx","app_secret":"xxx","verification_token":"xxx"}}'
```

---

### `PUT /api/channels/{id}`

更新渠道配置。

**路径参数**:

| 参数 | 类型 | 说明 |
|------|------|------|
| `id` | string (UUID) | 渠道 ID |

**请求体** (所有字段可选):

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 新名称 |
| `enabled` | boolean | 是否启用 |
| `config` | object | 新配置 JSON |

**请求示例**:
```json
{
  "enabled": true,
  "config": {
    "app_id": "cli_new",
    "app_secret": "new_secret",
    "verification_token": "new_token"
  }
}
```

**响应** `200 OK`: 更新后的完整渠道对象

**curl 示例**:
```bash
curl -X PUT http://localhost:3000/api/channels/ch-uuid-1 \
  -H "Content-Type: application/json" \
  -d '{"enabled":true}'
```

---

### `DELETE /api/channels/{id}`

删除渠道。

**响应** `200 OK`:
```json
{
  "deleted": true
}
```

**curl 示例**:
```bash
curl -X DELETE http://localhost:3000/api/channels/ch-uuid-1
```

---

### `POST /api/channels/{id}/test`

测试渠道连接状态（当前为 stub 实现）。

**响应** `200 OK`:
```json
{
  "channel_id": "ch-uuid-1",
  "status": "ok",
  "message": "Channel test not yet implemented"
}
```

**curl 示例**:
```bash
curl -X POST http://localhost:3000/api/channels/ch-uuid-1/test
```

---

### `POST /api/channels/feishu/callback`

飞书事件订阅回调端点。**此端点不需要客户端认证**，供飞书服务器调用。

**处理流程**:
1. URL 验证 (Challenge Echo): 如果请求体含 `challenge` 字段，直接返回 `{"challenge": "..."}`
2. 事件解密: 如果配置了 `encrypt_key`，使用 AES-256-CBC 解密 `encrypt` 字段
3. 消息解析: 提取 `event.message.content` (支持 text 和 post 类型)
4. 限流检查: 每群 30 条/分钟
5. AI 处理: 通过 SessionManager 生成回复
6. 发送回复: 文本消息或卡片消息（含代码块时）

**响应** `200 OK`: 总是返回 `{"code":0,"msg":"ok"}` 或类似格式（飞书要求在 3 秒内响应）

**注意**: AI 处理是异步的，回调函数在 3 秒内返回确认后，通过飞书发消息 API 单独发送回复。

---

## 6. Workflows

### `GET /api/workflows`

列出所有工作流定义。

**响应** `200 OK`:
```json
[
  {
    "id": "wf-uuid-1",
    "name": "Daily Report",
    "description": "Generate daily summary report",
    "definition": "{\"steps\":[...],\"edges\":[...]}",
    "trigger_type": "cron",
    "cron_expression": "0 9 * * *",
    "enabled": true,
    "last_run_at": "2026-06-19T09:00:00+00:00",
    "created_at": "2026-06-18T12:00:00+00:00",
    "updated_at": "2026-06-19T09:00:00+00:00"
  }
]
```

**curl 示例**:
```bash
curl http://localhost:3000/api/workflows
```

---

### `POST /api/workflows`

创建工作流。

**请求体**:

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `name` | string | 是 | - | 工作流名称 |
| `description` | string | 否 | `""` | 描述 |
| `definition` | object | 否 | - | 工作流定义 (DAG) |
| `trigger_type` | string | 否 | `"manual"` | 触发类型: `manual` 或 `cron` |
| `cron_expression` | string | 否 | null | Cron 表达式 (trigger_type=cron 时必填) |

**definition Schema**:
```typescript
{
  name: string;
  description: string;
  steps: WorkflowStep[];
  edges: WorkflowEdge[];
}

// WorkflowStep
{
  id: string;
  name: string;
  type: "llm_call" | "tool_call" | "publish" | "condition" | "delay";
  config: object;    // 步骤配置 (prompt, model, seconds, etc.)
  position?: { x: number; y: number };  // 编辑器位置
}

// WorkflowEdge
{
  id: string;
  source: string;     // 源步骤 ID
  target: string;     // 目标步骤 ID
  label?: string;
  condition?: string;
}
```

**请求示例**:
```json
{
  "name": "Morning Brief",
  "description": "搜新闻并总结",
  "trigger_type": "cron",
  "cron_expression": "0 8 * * *",
  "definition": {
    "name": "Morning Brief",
    "description": "搜新闻并总结",
    "steps": [
      {
        "id": "step-1",
        "name": "Search News",
        "type": "tool_call",
        "config": { "tool": "web_search", "query": "latest AI news" }
      },
      {
        "id": "step-2",
        "name": "Summarize",
        "type": "llm_call",
        "config": { "prompt": "Summarize this news", "model": "deepseek-chat" }
      },
      {
        "id": "step-3",
        "name": "Publish",
        "type": "publish",
        "config": {}
      }
    ],
    "edges": [
      { "id": "e1", "source": "step-1", "target": "step-2" },
      { "id": "e2", "source": "step-2", "target": "step-3" }
    ]
  }
}
```

**响应** `200 OK`: 创建的工作流对象（含自动生成的 UUID）

**curl 示例**:
```bash
curl -X POST http://localhost:3000/api/workflows \
  -H "Content-Type: application/json" \
  -d '{"name":"Test Workflow","trigger_type":"manual","definition":{"steps":[],"edges":[]}}'
```

---

### `GET /api/workflows/{id}`

获取单个工作流详情。

**路径参数**:

| 参数 | 类型 | 说明 |
|------|------|------|
| `id` | string (UUID) | 工作流 ID |

**响应** `200 OK`: 参见创建响应格式

**curl 示例**:
```bash
curl http://localhost:3000/api/workflows/wf-uuid-1
```

---

### `PUT /api/workflows/{id}`

更新工作流。所有字段可选。

**请求体**:

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | 否 | 新名称 |
| `description` | string | 否 | 新描述 |
| `definition` | object | 否 | 新工作流定义 |
| `trigger_type` | string | 否 | 触发类型 |
| `cron_expression` | string | 否 | Cron 表达式 |
| `enabled` | boolean | 否 | 是否启用 |

**响应** `200 OK`: 更新后的完整工作流对象

**curl 示例**:
```bash
curl -X PUT http://localhost:3000/api/workflows/wf-uuid-1 \
  -H "Content-Type: application/json" \
  -d '{"enabled":false}'
```

---

### `DELETE /api/workflows/{id}`

删除工作流。

**响应** `200 OK`:
```json
{
  "deleted": true
}
```

**curl 示例**:
```bash
curl -X DELETE http://localhost:3000/api/workflows/wf-uuid-1
```

---

### `POST /api/workflows/{id}/run`

手动执行工作流。系统会解析定义、执行 DAG 拓扑排序，并返回执行结果。

**响应** `200 OK`:
```json
{
  "workflow_id": "wf-uuid-1",
  "run_id": "run-uuid-1",
  "status": "success",
  "steps": [
    {
      "step_id": "step-1",
      "status": "success",
      "output": "Search results here...",
      "error": null
    },
    {
      "step_id": "step-2",
      "status": "success",
      "output": "Summary text...",
      "error": null
    }
  ],
  "publish_url": null
}
```

**步骤状态** (`status`): `pending`, `running`, `success`, `error`, `skipped`

**整体状态**:
- `success` -- 所有步骤成功
- `error` -- 至少一个步骤失败
- `skipped` -- 所有步骤被跳过（空工作流或上游失败）

**curl 示例**:
```bash
curl -X POST http://localhost:3000/api/workflows/wf-uuid-1/run
```

---

### `GET /api/workflows/{id}/runs`

获取工作流的执行历史记录。

**查询参数**:

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `limit` | integer | `20` | 返回记录数上限 |

**响应** `200 OK`:
```json
[
  {
    "id": "run-uuid-1",
    "workflow_id": "wf-uuid-1",
    "status": "success",
    "started_at": "2026-06-19T09:00:00+00:00",
    "finished_at": "2026-06-19T09:00:15+00:00",
    "result": "{\"steps\":[...]}",
    "publish_url": null
  }
]
```

**curl 示例**:
```bash
curl "http://localhost:3000/api/workflows/wf-uuid-1/runs?limit=10"
```

---

## 7. Tasks

### `GET /api/tasks`

列出所有定时任务。

**响应** `200 OK`:
```json
[
  {
    "id": "task-uuid-1",
    "name": "Hourly Weather Check",
    "cron_expression": "0 * * * *",
    "prompt": "Check the current weather in Beijing",
    "session_id": null,
    "model": "deepseek-chat",
    "enabled": true,
    "created_at": "2026-06-19T08:00:00+00:00",
    "updated_at": "2026-06-19T10:00:00+00:00"
  }
]
```

**curl 示例**:
```bash
curl http://localhost:3000/api/tasks
```

---

### `POST /api/tasks`

创建定时任务。

**请求体**:

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `name` | string | 是 | - | 任务名称 |
| `cron_expression` | string | 是 | - | Cron 表达式 (5 字段) |
| `prompt` | string | 是 | - | 发送给 LLM 的提示词 |
| `session_id` | string | 否 | null | 关联会话 ID |
| `model` | string | 否 | `"deepseek-chat"` | 使用的模型 |
| `enabled` | boolean | 否 | `true` | 是否启用 |

**Cron 表达式格式** (标准 5 字段):
```
┌──────── 分钟 (0-59)
│ ┌────── 小时 (0-23)
│ │ ┌──── 日期 (1-31)
│ │ │ ┌── 月份 (1-12)
│ │ │ │ ┌ 星期 (0-6, 0=周日)
│ │ │ │ │
* * * * *
```

**常见示例**:
| 表达式 | 含义 |
|--------|------|
| `0 * * * *` | 每小时整点 |
| `0 9 * * *` | 每天早上 9 点 |
| `0 9 * * 1-5` | 工作日早上 9 点 |
| `*/30 * * * *` | 每 30 分钟 |
| `0 0 1 * *` | 每月 1 号午夜 |

**请求示例**:
```json
{
  "name": "Daily Morning Report",
  "cron_expression": "0 9 * * *",
  "prompt": "Summarize today's top AI news and research papers.",
  "model": "deepseek-reasoner",
  "enabled": true
}
```

**响应** `200 OK`: 创建的任务对象（含自动生成的 UUID）

**curl 示例**:
```bash
curl -X POST http://localhost:3000/api/tasks \
  -H "Content-Type: application/json" \
  -d '{"name":"Daily Report","cron_expression":"0 9 * * *","prompt":"Summarize todays top AI news"}'
```

---

### `GET /api/tasks/{id}`

获取单个任务详情。

**curl 示例**:
```bash
curl http://localhost:3000/api/tasks/task-uuid-1
```

---

### `PUT /api/tasks/{id}`

更新任务。所有字段可选。

**请求体**:

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | 新名称 |
| `cron_expression` | string | 新 Cron 表达式 |
| `prompt` | string | 新提示词 |
| `session_id` | string | 新关联会话 |
| `model` | string | 新模型 |
| `enabled` | boolean | 是否启用 |

**请求示例**:
```json
{
  "enabled": false
}
```

**curl 示例**:
```bash
curl -X PUT http://localhost:3000/api/tasks/task-uuid-1 \
  -H "Content-Type: application/json" \
  -d '{"enabled":false}'
```

---

### `DELETE /api/tasks/{id}`

删除任务。

**curl 示例**:
```bash
curl -X DELETE http://localhost:3000/api/tasks/task-uuid-1
```

---

### `POST /api/tasks/{id}/run`

手动触发任务立即执行一次。

**响应** `200 OK`:
```json
{
  "task_id": "task-uuid-1",
  "result": "Task execution placeholder"
}
```

> 注意：当前为占位实现，完整执行逻辑将在后续版本实现。

**curl 示例**:
```bash
curl -X POST http://localhost:3000/api/tasks/task-uuid-1/run
```

---

### `GET /api/tasks/{id}/logs`

获取任务执行日志。

**响应** `200 OK`:
```json
[
  {
    "id": "log-uuid-1",
    "task_id": "task-uuid-1",
    "status": "success",
    "output": "The weather in Beijing today is sunny...",
    "error": null,
    "started_at": "2026-06-19T09:00:00+00:00",
    "finished_at": "2026-06-19T09:00:05+00:00"
  }
]
```

**curl 示例**:
```bash
curl http://localhost:3000/api/tasks/task-uuid-1/logs
```

---

## 8. Monitor

### `GET /api/monitor`

获取完整的服务器监控数据。

**响应** `200 OK`:
```json
{
  "server": {
    "uptime_secs": 86400,
    "uptime_display": "1d 0h 0m 0s",
    "request_count": 1523,
    "active_sse_connections": 2
  },
  "data": {
    "total_sessions": 15,
    "total_messages": 340,
    "db_size_bytes": 1048576,
    "db_size_display": "1.0 MB",
    "memory_rss_bytes": 47185920,
    "memory_rss_display": "45.0 MB"
  },
  "llm": {
    "calls_total": 298,
    "calls_success": 290,
    "calls_error": 8
  },
  "channels": {
    "web": 200,
    "feishu": 80,
    "qq": 18
  },
  "recent_errors": [
    {
      "timestamp": "2026-06-19T10:25:00+00:00",
      "message": "Feishu AI error (session xxx): rate limited"
    }
  ]
}
```

**curl 示例**:
```bash
curl http://localhost:3000/api/monitor
```

---

### `POST /api/monitor/reset`

重置所有运行时监控计数器（请求计数、SSE 连接数、频道消息量、LLM 统计、错误日志）。

**响应** `200 OK`:
```json
{
  "status": "ok",
  "message": "All monitoring counters have been reset"
}
```

**curl 示例**:
```bash
curl -X POST http://localhost:3000/api/monitor/reset
```

---

### `GET /monitor`

监控仪表板 HTML 页面。包含自动刷新（每 3 秒）、实时的服务器/数据/LLM/频道统计，以及最近 20 条错误日志。

直接在浏览器中打开: `http://localhost:3000/monitor`

---

## 9. Publish

### `GET /p/{publish_id}`

查看已发布的工作流执行结果。返回一个 Markdown 渲染的 HTML 页面。

**路径参数**:

| 参数 | 类型 | 说明 |
|------|------|------|
| `publish_id` | string | 发布 ID（对应 workflow_runs 中的 record） |

**响应** `200 OK`: 包含 Markdown 渲染内容的完整 HTML 页面（包含 Inter 字体、暗色主题）

**未找到时**: 返回 404 样式的 HTML 页面（HTTP 状态码仍为 200）

**curl 示例**:
```bash
curl http://localhost:3000/p/run-uuid-1
```

---

### `DELETE /api/publish/{id}`

删除已发布的页面。

**响应** `200 OK`:
```json
{
  "deleted": true,
  "id": "run-uuid-1"
}
```

**curl 示例**:
```bash
curl -X DELETE http://localhost:3000/api/publish/run-uuid-1
```

---

## 附录 A: 各渠道 config Schema 参考

### 飞书 (feishu)

```typescript
{
  app_id: string;             // 飞书 App ID
  app_secret: string;         // 飞书 App Secret
  verification_token: string; // 事件订阅 Verification Token
  encrypt_key?: string;       // 可选: AES 加密密钥
}
```

### QQ Bot (qq)

```typescript
{
  app_id: string;        // QQ Bot App ID (机器人 ID)
  client_secret: string; // QQ Bot Client Secret
  bot_secret?: string;   // 可选: Bot Secret (用于 Webhook 签名)
}
```

### 企业微信 (wechat_work)

```typescript
{
  corp_id: string;         // 企业 ID
  corp_secret: string;     // 应用 Secret
  agent_id: string;        // 应用 Agent ID
  token: string;           // 回调 Token
  encoding_aes_key: string; // 回调 EncodingAESKey
}
```

---

## 附录 B: Cron 表达式速查

```
字段:     秒    分    时    日    月    周
必需:    可选   必填  必填  必填  必填  必填
范围:    0-59  0-59  0-23  1-31  1-12  0-6
```

当前版本使用 5 字段格式（从小程序开始），无需秒字段。

| 表达式 | 说明 |
|--------|------|
| `* * * * *` | 每分钟 |
| `0 * * * *` | 每小时整点 |
| `0 0 * * *` | 每天午夜 |
| `0 9 * * 1-5` | 工作日 9:00 |
| `0 0 1 * *` | 每月 1 日午夜 |
| `0 */6 * * *` | 每 6 小时 (0:00, 6:00, 12:00, 18:00) |
| `30 8 * * *` | 每天 8:30 |
| `0 0 * * 0` | 每周日午夜 |
