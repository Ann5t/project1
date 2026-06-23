#!/bin/bash
#
# docker-pull.sh - Pull latest AI Agent Docker image and restart services
#
# Usage:
#   ./docker-pull.sh              # Pull and restart (production mode)
#   ./docker-pull.sh --staging    # Pull and restart with staging compose file
#   ./docker-pull.sh --help       # Show this help message
#
# Prerequisites:
#   - Docker Engine 24+
#   - Docker Compose v2
#   - Authenticated to ghcr.io (see README)

set -euo pipefail

# ── Colors ──────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ── Trap ────────────────────────────────────────────────────────────
cleanup() {
    local exit_code=$?
    if [ $exit_code -ne 0 ]; then
        echo -e "${RED}[ERROR]${NC} Script failed with exit code $exit_code" >&2
    fi
    echo -e "${CYAN}[INFO]${NC} Finished at $(date '+%Y-%m-%d %H:%M:%S')"
    exit $exit_code
}
trap cleanup EXIT

# ── Help ────────────────────────────────────────────────────────────
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Pull the latest AI Agent Docker image from GitHub Container Registry
and restart services with docker compose.

Options:
  --staging       Use docker-compose.staging.yml instead of the default
  --compose-file  Specify a custom compose file path
  --help          Show this help message and exit

Examples:
  $(basename "$0")
  $(basename "$0") --staging
  $(basename "$0") --compose-file /opt/ai-agent/docker-compose.prod.yml
EOF
    exit 0
}

# ── Parse arguments ────────────────────────────────────────────────
COMPOSE_FILE=""
STAGING=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --help) usage ;;
        --staging)
            STAGING=true
            COMPOSE_FILE="docker-compose.staging.yml"
            shift
            ;;
        --compose-file)
            if [[ -z "${2:-}" ]]; then
                echo -e "${RED}[ERROR]${NC} --compose-file requires a path argument" >&2
                exit 1
            fi
            COMPOSE_FILE="$2"
            shift 2
            ;;
        *)
            echo -e "${RED}[ERROR]${NC} Unknown option: $1" >&2
            usage
            ;;
    esac
done

# ── Pre-flight checks ──────────────────────────────────────────────
echo -e "${CYAN}[INFO]${NC} Running pre-flight checks..."

if ! command -v docker &>/dev/null; then
    echo -e "${RED}[ERROR]${NC} Docker is not installed or not in PATH." >&2
    echo "  Install Docker first: https://docs.docker.com/engine/install/" >&2
    exit 1
fi

if ! docker info &>/dev/null; then
    echo -e "${RED}[ERROR]${NC} Docker daemon is not running or the current user" >&2
    echo "  does not have permission to access it." >&2
    echo "  Try: sudo usermod -aG docker \$USER && newgrp docker" >&2
    exit 1
fi

DOCKER_VERSION=$(docker version --format '{{.Server.Version}}' 2>/dev/null || echo "unknown")
echo -e "${GREEN}[OK]${NC} Docker $DOCKER_VERSION is running."

# ── Resolve compose file ───────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

if [ -z "$COMPOSE_FILE" ]; then
    COMPOSE_FILE="docker-compose.yml"
fi

# If the given compose file is a relative path, look in common locations
if [[ "$COMPOSE_FILE" != /* ]]; then
    if [ -f "$PROJECT_DIR/$COMPOSE_FILE" ]; then
        COMPOSE_FILE_PATH="$PROJECT_DIR/$COMPOSE_FILE"
    elif [ -f "$SCRIPT_DIR/$COMPOSE_FILE" ]; then
        COMPOSE_FILE_PATH="$SCRIPT_DIR/$COMPOSE_FILE"
    elif [ -f "$COMPOSE_FILE" ]; then
        COMPOSE_FILE_PATH="$COMPOSE_FILE"
    else
        echo -e "${RED}[ERROR]${NC} Compose file not found: $COMPOSE_FILE" >&2
        exit 1
    fi
else
    COMPOSE_FILE_PATH="$COMPOSE_FILE"
    if [ ! -f "$COMPOSE_FILE_PATH" ]; then
        echo -e "${RED}[ERROR]${NC} Compose file not found: $COMPOSE_FILE_PATH" >&2
        exit 1
    fi
fi

echo -e "${GREEN}[OK]${NC} Using compose file: $COMPOSE_FILE_PATH"

# ── Authenticate to ghcr.io (optional) ──────────────────────────────
# If GITHUB_TOKEN is set, use it. Otherwise assume the user is already
# authenticated via `docker login ghcr.io`.
if [ -n "${GITHUB_TOKEN:-}" ]; then
    echo -e "${CYAN}[INFO]${NC} Authenticating to ghcr.io using GITHUB_TOKEN..."
    echo "$GITHUB_TOKEN" | docker login ghcr.io -u "${GITHUB_USER:-deploy}" --password-stdin
fi

# ── Pull latest image ──────────────────────────────────────────────
echo -e "${CYAN}[INFO]${NC} Pulling latest AI Agent images..."
docker compose -f "$COMPOSE_FILE_PATH" pull

echo -e "${GREEN}[OK]${NC} Image pull completed."

# ── Restart services ───────────────────────────────────────────────
echo -e "${CYAN}[INFO]${NC} Restarting services..."
docker compose -f "$COMPOSE_FILE_PATH" up -d --remove-orphans

echo -e "${GREEN}[OK]${NC} Services are running."

# ── Verify ─────────────────────────────────────────────────────────
echo -e "${CYAN}[INFO]${NC} Verifying service health..."
sleep 3

RUNNING=$(docker compose -f "$COMPOSE_FILE_PATH" ps --status running --format '{{.Name}}' 2>/dev/null | wc -l)
TOTAL=$(docker compose -f "$COMPOSE_FILE_PATH" ps --format '{{.Name}}' 2>/dev/null | wc -l)

if [ "$RUNNING" -eq "$TOTAL" ] && [ "$TOTAL" -gt 0 ]; then
    echo -e "${GREEN}[OK]${NC} All $TOTAL service(s) are running."
else
    echo -e "${YELLOW}[WARN]${NC} Only $RUNNING of $TOTAL service(s) are running."
    echo -e "${YELLOW}[WARN]${NC} Check logs with: docker compose -f \"$COMPOSE_FILE_PATH\" logs --tail=50"
fi

# ── Summary ────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Deployment update complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo -e "  ${CYAN}Compose file:${NC} $COMPOSE_FILE_PATH"
if [ "$STAGING" = true ]; then
    echo -e "  ${CYAN}Profile:${NC}     staging"
else
    echo -e "  ${CYAN}Profile:${NC}     production"
fi
echo -e "  ${CYAN}Time:${NC}        $(date '+%Y-%m-%d %H:%M:%S')"
echo ""
echo -e "  ${CYAN}Useful commands:${NC}"
echo -e "  [3m  docker compose -f \"$COMPOSE_FILE_PATH\" logs --tail=100 -f${NC}"
echo -e "  [3m  docker compose -f \"$COMPOSE_FILE_PATH\" ps${NC}"
echo ""
