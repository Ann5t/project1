# AI Agent

基于 Rust 的高性能 AI Agent 软件，Docker 一键部署，Web 端配置，支持多渠道接入（飞书、QQ、企业微信）。

<p align="center">
  <img src="crates/agent-frontend/assets/logo.svg" alt="AI Agent Logo" width="120" />
</p>

<p align="center">
  <em>屏幕截图 — 暗色主题 Web 配置界面 (Linear/Vercel 风格)</em>
</p>

---

## 特性

- **Rust 构建** -- 高性能，低内存占用，< 50MB RSS
- **Docker 一键启动** -- `docker compose up -d` 即可运行
- **Web 配置界面** -- 深色主题，Linear/Vercel 风格，响应式设计
- **DeepSeek 接入** -- 支持 Flash (deepseek-chat) 和 Pro (deepseek-reasoner) 模型
- **飞书 Bot** -- 完整的企业自建应用接入，支持消息卡、加密、限流
- **QQ Bot** -- WebSocket 实时接收，@提及和私信双模式
- **企业微信 Bot** -- 支持消息推送和 markdown 卡片
- **Webhook Bot** -- 通用 HTTP 回调接入，自定义路径和密钥验证
- **Workflow 引擎** -- DAG 拓扑排序执行，图形化编排多步骤任务
- **定时任务** -- Cron 表达式定时执行，含执行日志
- **结果发布** -- 任务结果生成可访问的 Markdown 渲染网页
- **多会话管理** -- 独立 Agent 和对话历史，支持系统提示词自定义
- **流式输出** -- Server-Sent Events (SSE) 实时响应流
- **工具调用** -- 内置计算器、时间查询、网页搜索、文件读取、Shell 执行
- **监控仪表板** -- 实时服务器状态、LLM API 统计、频道消息量、错误日志
- **引导式配置向导** -- 首次使用分步配置引导
- **认证系统** -- Admin Token 认证，Bearer Token 保护 API 和管理面板
- **全局搜索** -- 跨会话全文搜索对话历史
- **数据导出** -- 会话/工作流运行结果导出为 Markdown、HTML、CSV、JSON
- **备份恢复** -- SQLite 数据库备份下载与恢复
- **Email 通知** -- 任务完成/执行失败邮件通知（SMTP 配置）
- **Prometheus Metrics** -- `/api/metrics` 端点暴露服务指标
- **WebSocket 推送** -- 实时事件推送（任务状态、工作流执行进度）
- **OpenAPI 文档** -- `/api/docs` Swagger UI，`/api/openapi.json` 规范文件
- **速率限制** -- 全局/聊天 API 速率限制，可配置阈值

---

## 架构图

```
+------------------------------------------------------------------+
|                          AI Agent System                          |
+------------------------------------------------------------------+

     Browser / Mobile                  Feishu / QQ / WeChat
          |                                  |
          v                                  v
  +---------------+                 +------------------+
  | agent-frontend|                 |  Channel Layer   |
  |  (SPA / JS)   |                 |  feishu / qq_bot |
  +-------+-------+                 |  wechat_work     |
          |                         +--------+---------+
          | HTTP / SSE                        |
          v                                   |
  +-------------------------------------------+---------+
  |                   agent-server                        |
  |  (Axum HTTP Server + Routes + Middleware)            |
  |                                                      |
  |  /api/chat       /api/sessions    /api/workflows     |
  |  /api/tasks      /api/config      /api/channels      |
  |  /api/monitor    /api/health      /p/{publish_id}    |
  +-----+----------------+------------------+------------+
        |                |                  |
        v                v                  v
  +----------+   +--------------+   +----------------+
  | agent-core|   |  agent-db    |   | TaskScheduler |
  | LLM Client|   |  SQLite+sqlx |   | Cron Engine   |
  | Tool Reg. |   |  Migrations  |   +----------------+
  | Workflow  |   |  Repos       |
  | Sessions  |   +--------------+
  +-----+-----+
        |
        v
  +--------------+
  | DeepSeek API |
  | (OpenAI compat|
  |  + DuckDuckGo)|
  +--------------+
```

### 数据流 (Chat 请求)

```
User Message
    |
    v
POST /api/chat  ----->  SessionManager.process_message()
    |                         |
    |                    Load session + history
    |                         |
    |                    Build ChatRequest
    |                         |
    |                    LLMClient.chat() -----> DeepSeek API
    |                         |
    |                    <-- ChatResponse
    |                         |
    |                    Tool calls? --- YES --> ToolRegistry.execute()
    |                         |                      |
    |                         |                 Return tool result
    |                         |                      |
    |                         +---> Continue loop <--+
    |                         |
    |                    Save assistant message
    |                         |
    v                    Return final text
JSON Response / SSE Stream
```

---

## 快速开始

### 前置条件

- [Docker](https://docs.docker.com/get-docker/) 与 Docker Compose
- 公网服务器（如需飞书 / QQ 接入）
- [DeepSeek API Key](https://platform.deepseek.com/api_keys)

### 一行命令启动

```bash
docker compose up -d
```

访问 `http://localhost:3000` 或你的服务器公网 IP:3000。

### 首次配置

1. 打开 Web UI，自动进入引导式配置向导
2. 填入 DeepSeek API Key（从 [platform.deepseek.com](https://platform.deepseek.com) 获取）
3. （可选）配置飞书 Bot 或 QQ Bot 接入
4. 开始对话

---

## 开发环境搭建

### 安装 Rust

```bash
# 推荐使用 rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Windows: 下载 rustup-init.exe 从 https://rustup.rs
```

### 克隆并运行

```bash
git clone <your-repo-url> ai-agent
cd ai-agent

# 安装 SQLx CLI (数据库迁移工具)
cargo install sqlx-cli

# 创建 .env 文件
cp .env.example .env

# 运行数据库迁移
DATABASE_URL="sqlite:data/agent.db?mode=rwc" sqlx migrate run --source crates/agent-db/src/migrations

# 启动开发服务器
cargo run

# 或者 watch 模式 (需要 cargo-watch)
cargo watch -x run
```

### 项目结构

```
ai-agent/
├── crates/
│   ├── agent-server/          # Axum HTTP Server + REST API
│   │   ├── src/
│   │   │   ├── main.rs        # 入口，路由注册，启动逻辑
│   │   │   ├── lib.rs         # 公共导出
│   │   │   ├── config.rs      # 环境变量配置
│   │   │   ├── error.rs       # API 错误类型 + IntoResponse
│   │   │   ├── state.rs       # AppState 共享状态
│   │   │   ├── middleware.rs  # 请求计数中间件
│   │   │   ├── routes/        # API 路由处理函数
│   │   │   │   ├── health.rs  # GET /api/health, /api/info
│   │   │   │   ├── chat.rs    # POST /api/chat, /api/chat/stream
│   │   │   │   ├── session.rs # CRUD /api/sessions
│   │   │   │   ├── config_api.rs # GET/PUT /api/config
│   │   │   │   ├── workflow.rs   # CRUD /api/workflows
│   │   │   │   ├── task.rs       # CRUD /api/tasks
│   │   │   │   ├── channel.rs    # CRUD /api/channels
│   │   │   │   ├── publish.rs    # GET /p/:id, DELETE /api/publish
│   │   │   │   └── monitor.rs    # GET /api/monitor, /monitor
│   │   │   └── channel/       # 渠道集成实现
│   │   │       ├── feishu.rs  # 飞书 Bot (事件解析/加密/卡消息/限流)
│   │   │       ├── qq_bot.rs  # QQ Bot (WebSocket/认证/消息解析)
│   │   │       └── wechat_work.rs # 企业微信 Bot
│   │   └── Cargo.toml
│   ├── agent-core/            # 核心逻辑
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── error.rs       # CoreError 枚举
│   │   │   ├── llm/           # LLM 客户端
│   │   │   │   ├── client.rs  # LlmClient trait + DeepSeekClient
│   │   │   │   ├── types.rs   # ChatRequest/Response, SSE 类型
│   │   │   │   └── stats.rs   # LLM API 调用计数器
│   │   │   ├── tool/          # 工具系统
│   │   │   │   ├── types.rs   # Tool trait + ToolCallRequest/Result
│   │   │   │   ├── registry.rs # ToolRegistry
│   │   │   │   └── builtin.rs  # Calculator, Time, WebSearch, ReadFile, Shell
│   │   │   ├── session/       # 会话管理
│   │   │   │   └── manager.rs # SessionManager (agent loop + streaming)
│   │   │   ├── workflow/      # 工作流引擎
│   │   │   │   ├── types.rs   # WorkflowDefinition, Step, Edge, Result
│   │   │   │   └── engine.rs  # WorkflowEngine (DAG 执行)
│   │   │   └── scheduler/     # 定时任务
│   │   │       └── engine.rs  # TaskSchedulerEngine
│   │   └── Cargo.toml
│   ├── agent-db/              # 数据库层
│   │   ├── src/
│   │   │   ├── lib.rs         # init_db, run_migrations
│   │   │   ├── error.rs       # DbError
│   │   │   ├── models.rs       # 数据行类型 (Row structs)
│   │   │   ├── pool.rs         # SQLite 连接池
│   │   │   ├── repo/           # 仓库层 (CRUD)
│   │   │   │   ├── config_repo.rs
│   │   │   │   ├── session_repo.rs
│   │   │   │   ├── message_repo.rs
│   │   │   │   ├── channel_repo.rs
│   │   │   │   ├── workflow_repo.rs
│   │   │   │   └── task_repo.rs
│   │   │   └── migrations/     # SQL 迁移文件
│   │   │       ├── 001_initial.sql
│   │   │       └── 002_seed_config.sql
│   │   └── Cargo.toml
│   └── agent-frontend/        # 前端 SPA
│       ├── index.html
│       ├── js/api.js          # API 客户端 + 工具函数
│       └── assets/logo.svg
├── Dockerfile                 # Multi-stage build (cargo-chef)
├── docker-compose.yml         # 一键部署
├── .env.example               # 环境变量示例
└── README.md
```

---

## 环境变量

### 服务器配置

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `BIND_ADDRESS` | `0.0.0.0:3000` | 服务监听地址与端口 |
| `BIND_PORT` | `3000` | 备用端口指定（Docker Compose 中使用） |
| `DATABASE_PATH` | `data/agent.db` | SQLite 数据库文件路径 |
| `FRONTEND_DIR` | `crates/agent-frontend` | 前端静态文件目录 |
| `RUST_LOG` | `info` | 日志级别 (trace, debug, info, warn, error) |

### 安全与速率限制

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `CORS_ORIGIN` | (空) | CORS 允许的来源，逗号分隔多个。留空表示允许所有来源 |
| `REQUEST_TIMEOUT_SECS` | `60` | HTTP 请求超时秒数 |

### 工具安全

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `ALLOWED_DATA_DIR` | `data` | ReadFileTool 允许访问的目录 |
| `SHELL_TOOL_ENABLED` | `false` | 是否启用 ExecuteShellTool（安全风险，生产慎用） |
| `ALLOWED_COMMANDS` | (空) | ExecuteShellTool 命令白名单，逗号分隔 |
| `SHELL_TIMEOUT_SECS` | `30` | ExecuteShellTool 单命令超时秒数 |

### 运行时可配置项（通过 Web UI 或 API 设置，存入数据库）

这些配置不通过环境变量设置，而是在 Web UI 的「设置」页面或通过 API 动态修改：

| 配置键 | 默认值 | 说明 |
|--------|--------|------|
| `api_key` | (空) | DeepSeek API Key |
| `model` | `deepseek-chat` | 默认模型 (deepseek-chat / deepseek-reasoner) |
| `system_prompt` | (内置) | 默认系统提示词 |
| `temperature` | `0.7` | LLM 温度参数 |
| `max_tokens` | `4096` | LLM 最大输出 Token 数 |
| `admin_token` | (自动生成) | API 认证 Token |
| `auth_enabled` | `false` | 是否启用 API 认证 |
| `rate_limit_enabled` | `true` | 是否启用速率限制 |
| `rate_limit_global_rpm` | `100` | 全局每分钟请求上限 |
| `rate_limit_chat_rpm` | `10` | 每个 IP 聊天每分钟请求上限 |
| `smtp_host` | (空) | SMTP 服务器地址 |
| `smtp_port` | `587` | SMTP 端口 |
| `smtp_username` | (空) | SMTP 认证用户名 |
| `smtp_password` | (空) | SMTP 认证密码 |
| `smtp_from` | (空) | 邮件发件人地址 |
| `smtp_to` | (空) | 邮件通知收件人地址 |
| `smtp_tls` | `true` | SMTP 是否启用 TLS |
| `backup_interval_hours` | `24` | 自动备份间隔（小时），0 禁用 |
| `backup_retention_count` | `7` | 最多保留备份文件数 |

---

## 生产环境部署

### Docker Compose (推荐)

```bash
# 1. 准备环境变量
cp .env.example .env
# 编辑 .env 文件：
#   BIND_ADDRESS=0.0.0.0:3000
#   DATABASE_PATH=/app/data/agent.db
#   RUST_LOG=info

# 2. 构建并启动
docker compose build
docker compose up -d

# 3. 检查日志
docker compose logs -f

# 4. 查看监控仪表板
# 打开 http://<你的IP>:3000/monitor
```

### 直接部署 (无 Docker)

```bash
# 编译 release 版本
cargo build --release

# 确保 data/ 目录存在
mkdir -p data

# 运行迁移
DATABASE_URL="sqlite:data/agent.db?mode=rwc" \
  sqlx migrate run --source crates/agent-db/src/migrations

# 启动
BIND_ADDRESS=0.0.0.0:3000 \
DATABASE_PATH=data/agent.db \
FRONTEND_DIR=crates/agent-frontend \
RUST_LOG=info \
  ./target/release/agent-server
```

### 反向代理 (Nginx)

```nginx
server {
    listen 80;
    server_name ai.example.com;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # SSE 支持
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 3600s;
    }
}
```

### 健康检查

```bash
curl http://localhost:3000/api/health
# {"status":"ok","version":"0.1.0","timestamp":"2026-06-19T..."}

curl http://localhost:3000/api/info
# {"version":"0.1.0","status":"running","stats":{...},"tools":[...]}
```

---

## 飞书 Bot 接入教程

### 第 1 步：创建飞书应用

1. 登录 [飞书开发者后台](https://open.feishu.cn/)
2. 点击「创建企业自建应用」
3. 填写应用名称（如 "AI Agent"）和描述
4. 创建完成后进入应用详情页

### 第 2 步：获取凭证

在应用详情页左侧导航：

1. **凭证与基础信息**：
   - 复制 `App ID`
   - 复制 `App Secret`

2. **事件订阅**（稍后配置）：
   - 获取 `Verification Token`

### 第 3 步：配置权限

在「权限管理」中搜索并添加以下权限：

| 权限名称 | 说明 | 代码 |
|---------|------|------|
| 获取用户发给机器人的单聊消息 | 接收私信 | `im:message.p2p_msg:readonly` |
| 读取群聊中用户 @ 机器人的消息 | 接收群聊 @ | `im:message.group_at_msg:readonly` |
| 获取群组信息 | 读取群信息 | `im:chat:readonly` |
| 以应用身份发送消息 | 发送回复 | `im:message:send_as_bot` |

点击「批量开通」后确认。

### 第 4 步：配置事件订阅

1. 在应用详情页左侧点击「事件订阅」
2. 开启事件订阅功能
3. **请求地址 URL** 填入：
   ```
   http://你的公网IP:3000/api/channels/feishu/callback
   ```
   或（使用域名）：
   ```
   https://ai.example.com/api/channels/feishu/callback
   ```
4. **订阅事件**：点击「添加事件」，搜索并添加：
   - `im.message.receive_v1` -- 接收消息

5. 保存配置。飞书会向你的服务器发送验证请求。

### 第 5 步：发布应用

1. 在「应用发布」中点击「创建版本」
2. 填写版本号和更新说明
3. 提交审核（企业自建应用通常自动通过）
4. 审核通过后，点击「发布」

### 第 6 步：在 AI Agent 中配置

1. 打开 AI Agent Web UI (`http://你的IP:3000`)
2. 进入「设置 -> 渠道接入」
3. 点击「添加渠道」，选择「飞书」
4. 填入以下信息：
   ```json
   {
     "app_id": "cli_xxxxxxxxxxxxx",
     "app_secret": "xxxxxxxxxxxxxxxxxxxxxx",
     "verification_token": "xxxxxxxxxxxxxxxxxxxxxx",
     "encrypt_key": ""
   }
   ```
   - 如果不使用加密，`encrypt_key` 留空
   - 如果启用了加密，从飞书后台「事件订阅」获取 Encrypt Key 填入
5. 点击「保存」，然后启用该渠道
6. 点击「测试」验证连接状态

### 第 7 步：在飞书中测试

1. 在飞书中搜索你的应用名称
2. 打开对话，发送一条消息
3. AI Agent 会自动回复

### 配置了加密？

如果你的飞书应用启用了事件加密：

1. 在飞书后台「事件订阅」中获取 `Encrypt Key`
2. 在 AI Agent 渠道配置中填入 `encrypt_key` 字段
3. 保存后，系统会自动使用 AES-256-CBC 解密回调事件
4. 加密模式不支持 URL 验证 challenge -- 需要手动 URL 验证通过后再启用加密

---

## QQ Bot 接入教程

QQ Bot 使用 WebSocket 协议进行实时消息接收。

### 第 1 步：注册 QQ 开放平台

1. 登录 [QQ 开放平台](https://q.qq.com/)
2. 完成实名认证（个人或企业）
3. 进入「应用管理」创建机器人应用

### 第 2 步：获取凭证

1. 在应用详情页获取：
   - **机器人 ID** (App ID / BotAppID)
   - **Client Secret** (App Secret)
   - **Bot Secret** (用于 Webhook 签名验证，可选)

### 第 3 步：配置机器人权限

1. 在「开发设置」中配置：
   - **消息接收模式**：选择 WebSocket 模式
   - **订阅事件**：勾选
     - `C2C_MESSAGE_CREATE` -- 私聊消息
     - `GROUP_AT_MESSAGE_CREATE` -- 群聊 @机器人消息
2. 配置机器人的 QQ 号（由平台分配）

### 第 4 步：配置沙箱 / 上线

1. 开发阶段可使用 **沙箱环境** 测试
2. 沙箱中可添加测试成员
3. 测试通过后提交审核上线

### 第 5 步：在 AI Agent 中配置

1. 打开 AI Agent Web UI
2. 进入「设置 -> 渠道接入」
3. 点击「添加渠道」，选择「QQ」
4. 填入以下信息：
   ```json
   {
     "app_id": "10xxxxxxxx",
     "client_secret": "xxxxxxxxxxxx",
     "bot_secret": "xxxxxxxxxxxx"
   }
   ```
5. 点击「保存」，启用渠道

### 第 6 步：启动并测试

1. AI Agent 服务器启动后会自动：
   - 获取 Access Token
   - 发现 WebSocket 网关 URL
   - 建立 WebSocket 连接
   - 发送 Identify 报文
   - 开始监听消息事件
2. 在 QQ 中找到你的机器人
3. 在群聊中 @机器人发送消息，或在私聊中直接发送
4. 机器人会自动回复

### WebSocket 连接状态

通过监控仪表板 (`/monitor`) 查看：
- 频道消息量统计
- 最近的错误日志（含 WebSocket 断开/重连信息）

---

## 企业微信 Bot 接入教程

企业微信 Bot 通过回调 URL 接收消息，支持消息推送和 Markdown 卡片回复。

### 第 1 步：创建企业微信应用

1. 登录 [企业微信管理后台](https://work.weixin.qq.com/)
2. 进入「应用管理」->「自建」->「创建应用」
3. 填写应用名称和描述，选择可见范围
4. 创建完成后进入应用详情页

### 第 2 步：获取凭证

在应用详情页：

1. 复制 **AgentId**（应用 AgentId）
2. 复制 **Secret**（应用 Secret）
3. 在「企业信息」页获取 **CorpID**（企业 ID）

### 第 3 步：配置接收消息

在应用详情页：

1. 点击「接收消息」->「设置 API 接收」
2. **URL** 填入：
   ```
   http://你的公网IP:3000/api/channels/wechat_work/callback
   ```
3. **Token** 和 **EncodingAESKey** 可以随机生成（系统会自动处理验证）
4. 点击保存。企业微信会向你的服务器发送验证请求。

### 第 4 步：在 AI Agent 中配置

1. 打开 AI Agent Web UI
2. 进入「设置 -> 渠道接入」
3. 点击「添加渠道」，选择「企业微信」
4. 填入以下信息：
   ```json
   {
     "corp_id": "wwxxxxxxxxxxxxxx",
     "agent_id": "1000002",
     "secret": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
     "token": "xxxxxxxxxxxxxx",
     "encoding_aes_key": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
   }
   ```
5. 点击「保存」，启用渠道
6. 在企业微信中找到你的应用，发送消息测试

### 第 5 步：配置企业微信权限

确保应用拥有以下权限：
- **企业通讯录** -- 读取成员信息
- **消息** -- 接收消息和发送消息

在「企业可信 IP」中，可配置服务器 IP 来提高安全性。

---

## Webhook 通用回调接入

Webhook 渠道支持任意 HTTP 客户端通过 POST 请求向 Agent 发送消息，适合对接自定义系统或脚本。

### 第 1 步：创建 Webhook 渠道

1. 打开 AI Agent Web UI
2. 进入「设置 -> 渠道接入」
3. 点击「添加渠道」，选择「Webhook」
4. 配置参数：
   ```json
   {
     "path": "my-webhook",
     "secret": "your-shared-secret",
     "description": "自定义 Webhook"
   }
   ```
   - `path`: 回调 URL 路径（不可与其他渠道重复）
   - `secret`: 可选，用于 HMAC-SHA256 签名验证
5. 点击「保存」，启用渠道

### 第 2 步：调用 Webhook

**回调 URL**:
```
POST http://你的IP:3000/api/channels/webhook/{path}
```

**请求格式**:
```json
{
  "message": "你好，Agent！",
  "user_id": "optional-user-identifier"
}
```

**带签名的请求** (如果配置了 secret):
```bash
TIMESTAMP=$(date +%s)
BODY='{"message":"Hello"}'
SIGNATURE=$(echo -n "${TIMESTAMP}${BODY}" | openssl dgst -sha256 -hmac "your-shared-secret" | cut -d' ' -f2)

curl -X POST http://你的IP:3000/api/channels/webhook/my-webhook \
  -H "Content-Type: application/json" \
  -H "X-Signature: ${SIGNATURE}" \
  -H "X-Timestamp: ${TIMESTAMP}" \
  -d "${BODY}"
```

服务器会验证签名时间戳（5 分钟内有效），验证通过后处理消息并返回 Agent 回复。

---

## 认证与安全

### Admin Token

AI Agent 使用 Bearer Token 进行 API 认证：

1. **自动生成**: 首次启动时，系统自动生成 `admin_token` 并存入数据库
2. **查看 Token**: 在 Web UI「设置 -> 系统配置」中查看 `admin_token`
3. **启用认证**: 在配置中设置 `auth_enabled = true` 以启用认证
4. **发送请求**: 客户端在请求头携带 `Authorization: Bearer <admin_token>`

```bash
# 示例：使用 Token 访问 API
curl http://localhost:3000/api/config \
  -H "Authorization: Bearer your-admin-token-here"
```

### 速率限制

速率限制可在 Web UI 中配置（「设置 -> 系统配置」）：

| 配置键 | 默认值 | 说明 |
|--------|--------|------|
| `rate_limit_enabled` | `true` | 是否启用全局速率限制 |
| `rate_limit_global_rpm` | `100` | 全局每分钟请求上限 |
| `rate_limit_chat_rpm` | `10` | 每个 IP 每分钟聊天请求上限 |

超出限制时返回 HTTP `429 Too Many Requests`。

### CORS 配置

通过环境变量 `CORS_ORIGIN` 设置允许的跨域来源：

```env
# 允许单个来源
CORS_ORIGIN=https://ai.example.com

# 允许多个来源（逗号分隔）
CORS_ORIGIN=https://app1.example.com,https://app2.example.com

# 留空则允许所有来源（仅开发环境推荐）
CORS_ORIGIN=
```

---

## 速率限制

速率限制可在 Web UI 中配置（「设置 -> 系统配置」）：

| 配置键 | 默认值 | 说明 |
|--------|--------|------|
| `rate_limit_enabled` | `true` | 是否启用全局速率限制 |
| `rate_limit_global_rpm` | `100` | 全局每分钟请求上限 |
| `rate_limit_chat_rpm` | `10` | 每个 IP 每分钟聊天请求上限 |

超出限制时返回 HTTP `429 Too Many Requests`。

---

## 数据备份与恢复

### 手动备份

通过 Web UI（「设置 -> 数据管理」）或 API 下载 SQLite 数据库备份：

```bash
# 下载备份
curl http://localhost:3000/api/backup \
  -H "Authorization: Bearer your-token" \
  -o agent-backup.db
```

### 手动恢复

```bash
# 上传备份文件恢复
curl -X POST http://localhost:3000/api/backup/restore \
  -H "Authorization: Bearer your-token" \
  -F "file=@agent-backup.db"
```

### 自动备份

系统启动后自动开启定时备份任务，默认每 24 小时执行一次。备份配置可在 Web UI 中调整。

---

## 数据导出

支持将会话记录和工作流运行结果导出为多种格式。

### 导出单个会话

```bash
# 导出为 Markdown
curl http://localhost:3000/api/export/session/{id}?format=markdown \
  -H "Authorization: Bearer your-token" \
  -o session.md

# 导出为 JSON
curl http://localhost:3000/api/export/session/{id}?format=json \
  -H "Authorization: Bearer your-token" \
  -o session.json
```

支持的格式：`json`、`markdown`、`html`、`csv`

### 批量导出

通过 Web UI 选择多个会话批量导出，自动打包为 ZIP 下载。

---

## Email 通知配置

配置 SMTP 邮件通知，在任务完成或失败时发送邮件。

在 Web UI「设置 -> 系统配置」中添加以下配置项：

| 配置键 | 必填 | 说明 |
|--------|------|------|
| `smtp_host` | 是 | SMTP 服务器地址（如 `smtp.gmail.com`） |
| `smtp_port` | 是 | SMTP 端口（如 `587`） |
| `smtp_username` | 是 | SMTP 用户名 |
| `smtp_password` | 是 | SMTP 密码或应用专用密码 |
| `smtp_from` | 是 | 发件人邮箱地址 |
| `smtp_to` | 是 | 收件人邮箱地址（任务通知目标） |
| `smtp_tls` | 否 | 是否启用 TLS（默认 `true`） |

配置完成后，点击「通知设置」中的「测试邮件」按钮验证配置。

---

## API 参考

### 通用说明

- **Base URL**: `http://localhost:3000/api`
- **Content-Type**: `application/json`
- **错误响应**: `{"error": "错误信息描述"}`

### Health

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/health` | 健康检查 |
| `GET` | `/api/info` | 系统信息（版本、会话数、工具列表等） |

### Config

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/config` | 获取所有配置 |
| `PUT` | `/api/config` | 批量更新配置 `{"key": "value", ...}` |
| `GET` | `/api/config/{key}` | 获取单个配置值 |
| `PUT` | `/api/config/{key}` | 设置单个配置 `{"value": "..."}` |

### Sessions

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/sessions` | 列出所有会话 |
| `POST` | `/api/sessions` | 创建会话 |
| `GET` | `/api/sessions/{id}` | 获取会话详情 |
| `PUT` | `/api/sessions/{id}` | 更新会话 |
| `DELETE` | `/api/sessions/{id}` | 删除会话（级联删除消息） |
| `GET` | `/api/sessions/{id}/messages` | 获取会话消息历史 |

### Chat

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST` | `/api/chat` | 发送消息（非流式） |
| `POST` | `/api/chat/stream` | 发送消息（SSE 流式） |

### Channels

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/channels` | 列出所有渠道 |
| `POST` | `/api/channels` | 创建渠道 |
| `PUT` | `/api/channels/{id}` | 更新渠道配置 |
| `DELETE` | `/api/channels/{id}` | 删除渠道 |
| `POST` | `/api/channels/{id}/test` | 测试渠道连接 |
| `POST` | `/api/channels/feishu/callback` | 飞书事件回调（无需认证） |

### Workflows

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/workflows` | 列出所有工作流 |
| `POST` | `/api/workflows` | 创建工作流 |
| `GET` | `/api/workflows/{id}` | 获取工作流详情 |
| `PUT` | `/api/workflows/{id}` | 更新工作流 |
| `DELETE` | `/api/workflows/{id}` | 删除工作流 |
| `POST` | `/api/workflows/{id}/run` | 手动执行工作流 |
| `GET` | `/api/workflows/{id}/runs` | 获取工作流执行历史 |

### Tasks

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/tasks` | 列出所有定时任务 |
| `POST` | `/api/tasks` | 创建定时任务 |
| `GET` | `/api/tasks/{id}` | 获取任务详情 |
| `PUT` | `/api/tasks/{id}` | 更新任务 |
| `DELETE` | `/api/tasks/{id}` | 删除任务 |
| `POST` | `/api/tasks/{id}/run` | 手动触发执行 |
| `GET` | `/api/tasks/{id}/logs` | 获取任务执行日志 |

### Auth

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/auth/status` | 查询认证状态（是否启用） |
| `POST` | `/api/auth/login` | 登录（需要 admin_token） |

### Search

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/search?q=&type=&page=&limit=` | 全局搜索对话历史 |

### Backup

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/backup` | 下载数据库备份 |
| `POST` | `/api/backup/restore` | 上传并恢复数据库备份 |
| `GET` | `/api/backup/list` | 列出已有备份文件 |

### Export

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/export/session/{id}?format=` | 导出单个会话（json/markdown/html/csv） |
| `GET` | `/api/export/workflow/{id}/runs?format=` | 导出工作流执行历史 |
| `POST` | `/api/export/bulk` | 批量导出多个会话（打包 ZIP） |

### Monitor

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/monitor` | 获取监控数据 (JSON) |
| `POST` | `/api/monitor/reset` | 重置所有监控计数器 |
| `GET` | `/api/monitor/timeseries` | 获取时序监控数据 |
| `GET` | `/monitor` | 监控仪表板 (HTML, 自动刷新) |

### WebSocket

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/ws` | WebSocket 实时事件推送（任务/工作流状态） |

### Metrics

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/metrics` | Prometheus 格式的服务指标 |

### OpenAPI

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/openapi.json` | OpenAPI 3.0 规范文件 |
| `GET` | `/api/docs` | Swagger UI 交互式文档 |

### Publish

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/p/{publish_id}` | 查看已发布的结果页 (HTML) |
| `DELETE` | `/api/publish/{id}` | 删除已发布的结果 |

### Notifications

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST` | `/api/notifications/test-email` | 发送测试邮件验证 SMTP 配置 |

完整的请求/响应 Schema 和 curl 示例请参阅 [API_DOCS.md](./API_DOCS.md)。

---

## 技术栈

| 层 | 技术 | 说明 |
|----|------|------|
| **Web 框架** | Axum 0.8 (Rust) | 基于 Tokio 的高性能异步 HTTP |
| **LLM 客户端** | DeepSeek API | OpenAI 兼容协议，流式 SSE 支持 |
| **数据库** | SQLite (sqlx) | 零配置，WAL 模式，外键约束 |
| **前端** | 原生 JS + CSS | 零依赖 SPA，Linear/Vercel 风格 |
| **WebSocket** | tokio-tungstenite | QQ Bot 实时连接 |
| **容器化** | Docker Multi-stage | cargo-chef 缓存依赖层 |
| **加密** | AES-256-CBC + SHA-256 | 飞书事件加解密 |
| **监控** | sysinfo + 自建仪表板 | 内存/SSE/API 统计/错误日志 |

---

## 疑难解答 (Troubleshooting)

### 服务无法启动

**症状**: `docker compose up -d` 后容器退出

**检查**:
```bash
# 查看日志
docker compose logs ai-agent

# 常见原因：
# 1. 端口冲突 — 修改 BIND_ADDRESS 为 0.0.0.0:3001
# 2. data/ 目录权限 — 确保 Docker 有写入权限
# 3. 数据库文件损坏 — 删除 data/agent.db 重新启动
```

### DeepSeek API 调用失败

**症状**: 聊天返回 "LLM API error: HTTP 401"

**检查**:
1. 确认 API Key 正确（在 Web UI 的「设置」中填入）
2. 检查 API Key 余额是否充足
3. 检查网络是否能访问 `https://api.deepseek.com`
4. 查看监控仪表板中 LLM API 的 Error Rate

### 飞书 Bot 不回复

**症状**: 飞书消息发送后无响应

**排查步骤**:
1. **验证网络可达性**：服务器必须能从公网访问
   ```bash
   curl http://你的公网IP:3000/api/health
   ```
2. **检查回调 URL**：在飞书后台「事件订阅」中确认 URL 正确
3. **查看服务器日志**：
   ```bash
   docker compose logs ai-agent | grep feishu
   ```
4. **验证凭证**：App ID / App Secret 是否正确
5. **检查权限**：确保所有 4 个权限都已开通
6. **查看监控仪表板**：`/monitor` 查看最近错误日志

### QQ Bot 不回复

**症状**: QQ 中 @机器人后无响应

**排查步骤**:
1. **确认 WebSocket 模式**：在 QQ 开放平台选择 WebSocket 模式
2. **检查 App ID / Client Secret** 是否正确
3. **查看日志**：
   ```bash
   docker compose logs ai-agent | grep "QQ"
   ```
   正常情况下会看到：
   ```
   QQ access token refreshed
   QQ Bot: WebSocket connected
   QQ WS: sent Identify
   ```
4. **确认事件类型**：
   - 私聊消息: `C2C_MESSAGE_CREATE`
   - 群聊 @消息: `GROUP_AT_MESSAGE_CREATE`
5. **沙箱环境**：确认测试 QQ 号已加入沙箱

### 数据库问题

**症状**: 配置丢失或数据损坏

**解决**:
```bash
# 备份数据库
cp data/agent.db data/agent.db.bak

# 删除并重建 (会丢失数据!)
rm data/agent.db

# 重新运行迁移
DATABASE_URL="sqlite:data/agent.db?mode=rwc" \
  sqlx migrate run --source crates/agent-db/src/migrations

# 重启服务
docker compose restart
```

### SSE 流式输出断开

**症状**: 浏览器中流式输出中断，显示 "Network error"

**解决**:
1. 如果使用 Nginx 反代，确保：
   ```nginx
   proxy_buffering off;
   proxy_cache off;
   proxy_read_timeout 3600s;
   ```
2. 检查客户端网络稳定性
3. 查看 `/monitor` 中的 Active SSE 连接数

### 端口被占用

**症状**: `Address already in use (os error 98)`

**解决**:
```bash
# 修改端口
docker compose down
# 编辑 .env，设置 BIND_ADDRESS=0.0.0.0:3001
docker compose up -d
```

---

## License

MIT
