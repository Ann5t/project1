#!/bin/bash
#
# first-time-setup.sh - Bootstrap an AI Agent server from scratch
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/your-org/ai-agent/main/deploy/first-time-setup.sh | bash
#   # or
#   ./first-time-setup.sh [--target /opt/ai-agent] [--repo URL]
#
# This script will:
#   1. Check prerequisites (Docker, curl, git)
#   2. Create the target directory (/opt/ai-agent by default)
#   3. Download docker-compose.yml from the project's GitHub repo
#   4. Copy .env.example to .env and prompt for required values
#   5. Pull images and start services

set -euo pipefail

# ── Configurable defaults ──────────────────────────────────────────
TARGET_DIR="${TARGET_DIR:-/opt/ai-agent}"
GITHUB_RAW_BASE="${GITHUB_RAW_BASE:-https://raw.githubusercontent.com}"
GITHUB_REPO="${GITHUB_REPO:-your-org/ai-agent}"
GITHUB_BRANCH="${GITHUB_BRANCH:-main}"
COMPOSE_FILE="docker-compose.yml"
ENV_EXAMPLE_FILE=".env.example"

# ── Colors ──────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# ── Trap ────────────────────────────────────────────────────────────
cleanup() {
    local exit_code=$?
    if [ $exit_code -ne 0 ]; then
        echo -e "${RED}✖${NC} Setup failed with exit code $exit_code" >&2
    fi
    exit $exit_code
}
trap cleanup EXIT

# ── Help ────────────────────────────────────────────────────────────
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Bootstrap an AI Agent server from scratch.

Options:
  --target DIR    Installation directory (default: $TARGET_DIR)
  --repo ORG/REPO GitHub repository (default: $GITHUB_REPO)
  --branch BRANCH Git branch to use (default: $GITHUB_BRANCH)
  --help          Show this help message and exit

Environment variables:
  TARGET_DIR       Same as --target
  GITHUB_REPO       Same as --repo
  GITHUB_BRANCH     Same as --branch
  GITHUB_TOKEN     GitHub personal access token (for private repos)

Examples:
  $(basename "$0") --repo my-org/ai-agent --branch release
  TARGET_DIR=/srv/ai-agent $(basename "$0")
EOF
    exit 0
}

# ── Parse arguments ────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --help) usage ;;
        --target) TARGET_DIR="$2"; shift 2 ;;
        --repo) GITHUB_REPO="$2"; shift 2 ;;
        --branch) GITHUB_BRANCH="$2"; shift 2 ;;
        *) echo -e "${RED}[ERROR]${NC} Unknown option: $1" >&2; usage ;;
    esac
done

# ── Banner ─────────────────────────────────────────────────────────
echo ""
echo -e "${CYAN}============================================${NC}"
echo -e "${CYAN}  AI Agent - First Time Server Setup${NC}"
echo -e "${CYAN}============================================${NC}"
echo ""
echo -e "  Target directory: ${BOLD}$TARGET_DIR${NC}"
echo -e "  GitHub repo:      ${BOLD}$GITHUB_REPO${NC}"
echo -e "  Branch:           ${BOLD}$GITHUB_BRANCH${NC}"
echo ""

# ── Step 0: Check prerequisites ────────────────────────────────────
echo -e "${CYAN}[1/6]${NC} Checking prerequisites..."

# Docker
if ! command -v docker &>/dev/null; then
    echo -e "${RED}[ERROR]${NC} Docker is not installed."
    echo "  Install Docker: https://docs.docker.com/engine/install/"
    echo ""
    echo "  Quick install (Ubuntu/Debian):"
    echo "    curl -fsSL https://get.docker.com | sh"
    echo "    sudo usermod -aG docker \$USER"
    echo "    newgrp docker"
    exit 1
fi

if ! docker info &>/dev/null; then
    echo -e "${RED}[ERROR]${NC} Docker daemon is not running or user lacks permissions."
    echo "  Try: sudo systemctl start docker"
    exit 1
fi
echo -e "${GREEN}  ✔${NC} Docker $(docker version --format '{{.Server.Version}}' 2>/dev/null) is running"

# curl
if ! command -v curl &>/dev/null; then
    echo -e "${RED}[ERROR]${NC} curl is not installed."
    echo "  Install: sudo apt install curl -y  (or your package manager)"
    exit 1
fi
echo -e "${GREEN}  ✔${NC} curl is available"

# git
if ! command -v git &>/dev/null; then
    echo -e "${YELLOW}  ⚠${NC} git is not installed (optional, only needed for cloning)"
fi

echo ""

# ── Step 1: Create target directory ───────────────────────────────
echo -e "${CYAN}[2/6]${NC} Creating target directory: $TARGET_DIR"

if [ -d "$TARGET_DIR" ]; then
    echo -e "${YELLOW}  ⚠${NC} Directory already exists. Files may be overwritten."
else
    sudo mkdir -p "$TARGET_DIR"
    echo -e "${GREEN}  ✔${NC} Directory created"
fi

cd "$TARGET_DIR"
echo ""

# ── Step 2: Download docker-compose.yml ───────────────────────────
echo -e "${CYAN}[3/6]${NC} Downloading docker-compose.yml..."

COMPOSE_URL="${GITHUB_RAW_BASE}/${GITHUB_REPO}/${GITHUB_BRANCH}/${COMPOSE_FILE}"
ENV_EXAMPLE_URL="${GITHUB_RAW_BASE}/${GITHUB_REPO}/${GITHUB_BRANCH}/${ENV_EXAMPLE_FILE}"

# If GITHUB_TOKEN is set, use it for authentication (private repos)
CURL_OPTS=(-fsSL)
if [ -n "${GITHUB_TOKEN:-}" ]; then
    CURL_OPTS=(-fsSL -H "Authorization: token $GITHUB_TOKEN")
fi

echo -e "  Downloading: $COMPOSE_URL"
if curl "${CURL_OPTS[@]}" -o "$TARGET_DIR/$COMPOSE_FILE" "$COMPOSE_URL"; then
    echo -e "${GREEN}  ✔${NC} $COMPOSE_FILE downloaded"
else
    echo -e "${RED}[ERROR]${NC} Failed to download $COMPOSE_URL" >&2
    echo "  Check:"
    echo "    - The repository URL is correct: $GITHUB_REPO"
    echo "    - The branch exists: $GITHUB_BRANCH"
    echo "    - The file exists in the repo: $COMPOSE_FILE"
    echo "    - For private repos, set GITHUB_TOKEN"
    exit 1
fi

echo ""

# ── Step 3: Download and configure .env ───────────────────────────
echo -e "${CYAN}[4/6]${NC} Setting up environment configuration..."

# Download .env.example
if curl "${CURL_OPTS[@]}" -o "$TARGET_DIR/.env.example" "$ENV_EXAMPLE_URL"; then
    echo -e "${GREEN}  ✔${NC} .env.example downloaded"
else
    echo -e "${YELLOW}  ⚠${NC} Could not download .env.example (creating empty template)"
    cat > "$TARGET_DIR/.env.example" <<- 'EOF'
# AI Agent - Environment Configuration
# Copy this file to .env and fill in the values.

# --- Application ---
APP_NAME=ai-agent
APP_ENV=production
APP_PORT=8080
LOG_LEVEL=info

# --- Database ---
DATABASE_URL=postgres://agent:changeme@db:5432/ai_agent

# --- Redis ---
REDIS_URL=redis://redis:6379

# --- Authentication ---
JWT_SECRET=change-this-to-a-random-64-char-string
API_KEY=

# --- AI Provider ---
OPENAI_API_KEY=
ANTHROPIC_API_KEY=

# --- GitHub Container Registry ---
# Leave empty for public images, set for private packages
GITHUB_TOKEN=
GITHUB_USER=
EOF
    echo -e "${GREEN}  ✔${NC} Created .env.example template"
fi

# If .env does not exist, create it from .env.example and prompt
if [ ! -f "$TARGET_DIR/.env" ]; then
    cp "$TARGET_DIR/.env.example" "$TARGET_DIR/.env"
    echo -e "${YELLOW}  ⚠${NC} A default .env file has been created from .env.example."
    echo ""
    echo -e "${BOLD}  Please edit $TARGET_DIR/.env and set the required values.${NC}"
    echo ""
    echo "  Required fields you MUST configure:"
    echo -e "    ${YELLOW}  - JWT_SECRET${NC}     (generate with: openssl rand -hex 32)"
    echo -e "    ${YELLOW}  - DATABASE_URL${NC}    (or use the default if deploying with bundled DB)"
    echo -e "    ${YELLOW}  - At least one AI provider key:${NC}"
    echo -e "         OPENAI_API_KEY or ANTHROPIC_API_KEY"
    echo ""
    echo -n "  Press Enter after you have configured .env, or type 'skip' to continue later: "
    read -r user_input
    if [ "$user_input" = "skip" ]; then
        echo -e "${YELLOW}  ⚠${NC} Skipping .env configuration. Services may not start correctly."
    else
        echo -e "${GREEN}  ✔${NC} .env configured"
    fi
else
    echo -e "${GREEN}  ✔${NC} .env already exists, keeping existing configuration"
fi
echo ""

# ── Step 4: Pull Docker images ─────────────────────────────────────
echo -e "${CYAN}[5/6]${NC} Pulling Docker images..."

# Authenticate to ghcr.io if token is set
if [ -n "${GITHUB_TOKEN:-}" ]; then
    echo "$GITHUB_TOKEN" | docker login ghcr.io -u "${GITHUB_USER:-deploy}" --password-stdin 2>/dev/null \
        && echo -e "${GREEN}  ✔${NC} Authenticated to ghcr.io" \
        || echo -e "${YELLOW}  ⚠${NC} ghcr.io authentication failed (proceeding anyway)"
fi

echo -e "  Pulling images (this may take a while)..."
cd "$TARGET_DIR"
docker compose -f "$TARGET_DIR/$COMPOSE_FILE" pull 2>&1 | while IFS= read -r line; do
    echo "  $line"
done

echo -e "${GREEN}  ✔${NC} Images pulled successfully"
echo ""

# ── Step 5: Start services ─────────────────────────────────────────
echo -e "${CYAN}[6/6]${NC} Starting services..."

cd "$TARGET_DIR"
docker compose -f "$TARGET_DIR/$COMPOSE_FILE" up -d --remove-orphans

echo -e "${GREEN}  ✔${NC} Services started"
echo ""

# ── Verify ─────────────────────────────────────────────────────────
echo -e "${CYAN}[INFO]${NC} Waiting for services to become healthy..."
sleep 5

cd "$TARGET_DIR"
docker compose ps

echo ""
echo -e "${GREEN}============================================${NC}"
echo -e "${GREEN}  Setup complete!${NC}"
echo -e "${GREEN}============================================${NC}"
echo ""
echo -e "  ${BOLD}Target directory:${NC} $TARGET_DIR"
echo -e "  ${BOLD}Compose file:${NC}     $TARGET_DIR/$COMPOSE_FILE"
echo -e "  ${BOLD}Configuration:${NC}    $TARGET_DIR/.env"
echo ""
echo -e "  ${CYAN}Useful commands:${NC}"
echo -e "    docker compose -f $TARGET_DIR/$COMPOSE_FILE logs --tail=50 -f"
echo -e "    docker compose -f $TARGET_DIR/$COMPOSE_FILE ps"
echo -e "    docker compose -f $TARGET_DIR/$COMPOSE_FILE down"
echo ""
echo -e "  ${YELLOW}Next steps:${NC}"
echo -e "    - Configure a reverse proxy (nginx / Caddy) if needed"
echo -e "    - Set up SSL certificates"
echo -e "    - Configure monitoring and alerts"
echo ""
