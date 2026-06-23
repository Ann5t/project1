# AI Agent Docker 部署指南

本文档介绍如何将 AI Agent 项目部署到服务器。提供三种部署方案，适用于不同的开发和运维场景。

---

## 目录

- [方案一：GitHub Actions 自动构建（推荐）](#方案一github-actions-自动构建推荐)
- [方案二：本地构建 + 上传镜像](#方案二本地构建--上传镜像)
- [方案三：服务器直接 git clone 构建](#方案三服务器直接-git-clone-构建)
- [阿里云镜像加速配置](#阿里云镜像加速配置)
- [安全组端口开放](#安全组端口开放)
- [环境变量说明](#环境变量说明)
- [常见故障排除](#常见故障排除)

---

## 方案一：GitHub Actions 自动构建（推荐）

利用 GitHub Actions CI/CD 流水线，在每次推送代码时自动构建 Docker 镜像并推送到 GitHub Container Registry (ghcr.io)，然后在服务器上拉取最新镜像并重启服务。

### 前提条件

1. 项目已托管在 GitHub
2. 服务器已安装 Docker 和 Docker Compose v2

### 配置步骤

#### 1. 在 GitHub 仓库添加 Secrets

进入 GitHub 仓库 -> `Settings` -> `Secrets and variables` -> `Actions`，添加以下 Secrets：

| Secret 名称 | 说明 |
|---|---|
| `DEPLOY_HOST` | 服务器 SSH 地址 |
| `DEPLOY_USER` | SSH 用户名 |
| `DEPLOY_SSH_KEY` | SSH 私钥 |
| `GITHUB_TOKEN` | 自动可用，无需手动添加 |

#### 2. 在服务器上准备部署目录

```bash
# 确保 $HOME/.ssh/authorized_keys 已配置部署公钥
sudo mkdir -p /opt/ai-agent
sudo chown -R $USER:$USER /opt/ai-agent
```

#### 3. 推送代码触发自动部署

每次推送代码到 `main` 或 `release` 分支时，GitHub Actions 会自动：

1. 构建 Rust 项目（多阶段构建，优化镜像大小）
2. 将 Docker 镜像推送到 `ghcr.io/your-org/ai-agent:latest`
3. 通过 SSH 在服务器上执行 `docker-pull.sh` 脚本拉取并重启

### 项目 GitHub Actions 工作流文件

工作流文件位于 `.github/workflows/deploy.yml`，主要内容包括：

- **构建阶段**：使用 `rust:1-slim-bookworm` 作为构建环境，编译 Rust 二进制文件
- **镜像阶段**：基于 `debian:bookworm-slim` 构建最小运行镜像
- **推送阶段**：将镜像推送到 `ghcr.io`
- **部署阶段**：通过 SSH 在服务器上执行部署脚本

---

## 方案二：本地构建 + 上传镜像

在没有 GitHub Actions CI/CD 的情况下，可以在本地开发机器上构建镜像，然后手动上传到服务器。

### 本地构建

```bash
# 1. 进入项目目录
cd ai-agent

# 2. 构建 Docker 镜像
docker build -t ai-agent:latest .

# 3. 为 ghcr.io 打标签
docker tag ai-agent:latest ghcr.io/your-org/ai-agent:latest
docker tag ai-agent:latest ghcr.io/your-org/ai-agent:$(git rev-parse --short HEAD)

# 4. 推送到 ghcr.io
docker push ghcr.io/your-org/ai-agent:latest
docker push ghcr.io/your-org/ai-agent:$(git rev-parse --short HEAD)
```

### 保存并传输镜像（无镜像仓库环境）

```bash
# 1. 保存镜像为 tar 文件
docker save ai-agent:latest | gzip > ai-agent-latest.tar.gz

# 2. 传输到服务器（通过 SCP 或 rsync）
scp ai-agent-latest.tar.gz user@your-server:/opt/ai-agent/

# 3. 在服务器上加载镜像
ssh user@your-server
cd /opt/ai-agent
docker load < ai-agent-latest.tar.gz

# 4. 重启服务
docker compose up -d
```

### 使用部署脚本

```bash
# 在服务器上执行部署拉取脚本
cd /opt/ai-agent
chmod +x deploy/docker-pull.sh
./deploy/docker-pull.sh
```

---

## 方案三：服务器直接 git clone 构建

适用于开发环境或无法使用预构建镜像的场景。在服务器上直接克隆仓库并构建 Docker 镜像。

### 步骤

```bash
# 1. 克隆仓库
git clone https://github.com/your-org/ai-agent.git /opt/ai-agent
cd /opt/ai-agent

# 2. 复制并配置环境变量
cp .env.example .env
# 编辑 .env 文件，填入必要的配置值
nano .env

# 3. 使用 Docker Compose 构建并启动
docker compose up -d --build

# 4. 查看运行状态
docker compose ps
docker compose logs --tail=50 -f
```

### 更新代码并重新构建

```bash
cd /opt/ai-agent
git pull origin main
docker compose up -d --build
```

---

## 阿里云镜像加速配置

在中国大陆访问 Docker Hub 和 ghcr.io 可能较慢，建议配置阿里云镜像加速器。

### 配置方法

#### Docker Desktop（Windows/Mac）

1. 打开 Docker Desktop -> `Settings` -> `Docker Engine`
2. 编辑 `registry-mirrors` 配置：

```json
{
  "registry-mirrors": [
    "https://<your-accel-id>.mirror.aliyuncs.com"
  ]
}
```

3. 点击 `Apply & Restart`

#### Linux 服务器

```bash
# 1. 创建或编辑 Docker 配置文件
sudo mkdir -p /etc/docker
sudo tee /etc/docker/daemon.json <<-'EOF'
{
  "registry-mirrors": [
    "https://<your-accel-id>.mirror.aliyuncs.com"
  ],
  "log-driver": "json-file",
  "log-opts": {
    "max-size": "10m",
    "max-file": "3"
  }
}
EOF

# 2. 重启 Docker
sudo systemctl daemon-reload
sudo systemctl restart docker
```

> **注意**：请将 `<your-accel-id>` 替换为阿里云容器镜像服务中的加速器地址。登录 [cr.console.aliyun.com](https://cr.console.aliyun.com) 获取专属加速地址。

### 针对 ghcr.io 的加速

阿里云镜像加速主要加速 Docker Hub。对于 `ghcr.io` 的镜像，可以通过以下方式加速：

1. **使用代理**：在服务器上配置 HTTP 代理
2. **通过阿里云容器镜像服务（ACR）同步**：将 ghcr.io 镜像同步到阿里云 ACR，然后修改 `docker-compose.yml` 中的镜像地址

---

## 安全组端口开放

部署前请确保云服务商的安全组或防火墙已开放以下端口：

| 端口 | 协议 | 用途 | 建议 |
|---|---|---|---|
| 22 | TCP | SSH 远程管理 | 仅允许特定 IP |
| 80 | TCP | HTTP（反向代理） | 对全网开放 |
| 443 | TCP | HTTPS（反向代理） | 对全网开放 |
| 8080 | TCP | 应用主服务端口 | 通过反向代理访问，不直接开放 |
| 5432 | TCP | PostgreSQL | 仅允许内网访问 |
| 6379 | TCP | Redis | 仅允许内网访问 |

### 安全建议

```bash
# 示例：仅允许内网访问数据库
# 使用 docker-compose 的网络隔离即可，无需在安全组开放数据库端口

# 限制 SSH 访问（阿里云安全组）
# 源：0.0.0.0/0 -> 建议改为你的办公网络 IP

# 安装并配置 fail2ban 防暴力破解
sudo apt install fail2ban -y
sudo systemctl enable fail2ban
sudo systemctl start fail2ban
```

---

## 环境变量说明

项目使用 `.env` 文件配置环境变量。以下列出所有可配置的变量：

| 变量名 | 必需 | 默认值 | 说明 |
|---|---|---|---|
| `APP_NAME` | 否 | `ai-agent` | 应用名称，用于日志和监控 |
| `APP_ENV` | 否 | `production` | 运行环境：`production`、`staging`、`development` |
| `APP_PORT` | 否 | `8080` | 应用监听端口 |
| `LOG_LEVEL` | 否 | `info` | 日志级别：`trace`、`debug`、`info`、`warn`、`error` |
| `RUST_LOG` | 否 | `info` | Rust 日志级别（覆盖 `LOG_LEVEL`） |
| `DATABASE_URL` | 是 | — | PostgreSQL 数据库连接字符串 |
| `REDIS_URL` | 否 | — | Redis 连接字符串（用于缓存和会话） |
| `JWT_SECRET` | **是** | — | JWT 签名密钥（至少 64 个随机字符） |
| `API_KEY` | 否 | — | 外部服务 API 密钥 |
| `OPENAI_API_KEY` | 推荐 | — | OpenAI API 密钥（使用 AI 功能时需要） |
| `ANTHROPIC_API_KEY` | 推荐 | — | Anthropic API 密钥（使用 Claude 时需要） |
| `GITHUB_TOKEN` | 否 | — | GitHub Personal Access Token（用于拉取私有镜像） |
| `GITHUB_USER` | 否 | `deploy` | ghcr.io 登录用户名 |

### 生成安全密钥

```bash
# 生成 64 字符随机 JWT 密钥
openssl rand -hex 32

# 或者使用更长的密钥
openssl rand -base64 48
```

### 环境验证

启动后验证环境配置是否正确：

```bash
# 检查容器环境变量
docker compose exec app env | grep -E '^(APP_|JWT_|DATABASE_URL|OPENAI|ANTHROPIC)'

# 查看应用日志确认启动正常
docker compose logs app --tail=30
```

---

## 常见故障排除

### 1. Docker 未安装或版本过低

```bash
# 检查版本
docker --version

# 安装最新 Docker
curl -fsSL https://get.docker.com | sh

# Ubuntu/Debian
sudo apt install docker.io docker-compose-v2

# CentOS/RHEL
sudo yum install docker docker-compose-plugin
```

### 2. Docker 守护进程未运行

```bash
sudo systemctl status docker
sudo systemctl start docker
sudo systemctl enable docker  # 设置开机自启
```

### 3. 权限不足（permission denied）

```bash
# 将当前用户加入 docker 组
sudo usermod -aG docker $USER

# 重新登录或执行
newgrp docker
```

### 4. ghcr.io 拉取镜像失败

```bash
# 确保已登录
echo $GITHUB_TOKEN | docker login ghcr.io -u your-username --password-stdin

# 检查镜像是否存在
docker pull ghcr.io/your-org/ai-agent:latest

# 网络问题：使用代理或镜像加速
export HTTP_PROXY=http://your-proxy:port
export HTTPS_PROXY=http://your-proxy:port
```

### 5. 容器启动后立即退出

```bash
# 查看容器日志
docker compose logs --tail=100

# 常见原因：
# - .env 文件缺少必要变量
# - 数据库连接失败
# - 端口被占用

# 检查端口占用
sudo lsof -i :8080
# 或
netstat -tlnp | grep 8080
```

### 6. 数据库连接失败

```bash
# 检查数据库容器是否正常
docker compose ps db

# 检查数据库日志
docker compose logs db --tail=50

# 测试连接（在应用容器内）
docker compose exec app bash -c "ping db"

# 确保 DATABASE_URL 格式正确
# postgres://username:password@hostname:port/database
```

### 7. 磁盘空间不足

```bash
# 查看磁盘使用情况
df -h

# 清理未使用的 Docker 资源
docker system prune -af

# 清理特定卷
docker volume prune -f

# 查看镜像和容器占用的空间
docker system df
```

### 8. 更新后服务不可用

```bash
# 回滚到上一个版本
docker compose down
# 使用之前的镜像标签重新启动
docker compose up -d

# 查看最近的变更
docker compose logs --since=10m

# 检查容器状态
docker compose ps -a
```

### 9. 端口冲突

```bash
# 检查占用端口的进程
sudo lsof -i :8080

# 修改 .env 中的 APP_PORT
# 或修改 docker-compose.yml 中的端口映射
```

### 10. 构建缓存问题

```bash
# 强制重新构建（不使用缓存）
docker compose build --no-cache

# 清理构建缓存
docker builder prune -af
```

---

## 快速参考

### 常用 Docker Compose 命令

```bash
# 启动服务（后台）
docker compose up -d

# 查看状态
docker compose ps

# 查看日志
docker compose logs --tail=50 -f

# 重启服务
docker compose restart

# 停止并移除容器
docker compose down

# 停止并移除容器和卷（⚠️ 会丢失数据）
docker compose down -v

# 重新构建并启动
docker compose up -d --build
```

### 健康检查

```bash
# 检查应用 HTTP 健康端点
curl -s http://localhost:8080/health | jq .

# 或使用 Docker 的健康检查状态
docker inspect --format='{{.State.Health.Status}}' ai-agent-app-1
```

### 备份与恢复

```bash
# 备份数据库
docker compose exec db pg_dump -U agent ai_agent > backup_$(date +%Y%m%d).sql

# 恢复数据库
cat backup.sql | docker compose exec -T db psql -U agent ai_agent
```

---

> **文档维护**：本指南适用于项目初始部署和日常运维。如有更新或问题，请提交 Issue 或 Pull Request。
