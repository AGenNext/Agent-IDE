#!/usr/bin/env bash
# Autonomyx Platform — one-file installer
# Usage:  curl -fsSL https://raw.githubusercontent.com/agennext/agent-ide/main/install.sh | bash
#         bash install.sh [--mode docker|binary] [--model <model>] [--port 3001] [--token <key>]
#
# Modes:
#   docker  (default) — pulls images from ghcr.io, runs via docker compose + systemd
#   binary             — downloads the pre-built autonomyx-runner binary, installs as systemd service
#
# Supports: AlmaLinux 8/9, RHEL 8/9, Rocky, Ubuntu 22+, Debian 11+, macOS 14+
set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────
INSTALL_MODE="docker"
INSTALL_DIR="/opt/autonomyx"
BINARY_DIR="/usr/local/bin"
BINARY_NAME="autonomyx-runner"
GITHUB_REPO="agennext/agent-ide"
IDE_IMAGE="ghcr.io/agennext/agent-ide:latest"
RUNNER_IMAGE="ghcr.io/agennext/autonomyx-runner:latest"
MODEL="claude-opus-4-8"
PORT_IDE=80
PORT_API=3001
TOKEN=""
NO_SERVICE=false
VERSION="latest"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'
info()    { echo -e "${CYAN}▸ $*${NC}"; }
success() { echo -e "${GREEN}✓ $*${NC}"; }
warn()    { echo -e "${YELLOW}⚠ $*${NC}"; }
die()     { echo -e "${RED}✗ $*${NC}" >&2; exit 1; }

# ── Args ──────────────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)       INSTALL_MODE="$2"; shift 2 ;;
    --model)      MODEL="$2";        shift 2 ;;
    --port)       PORT_API="$2";     shift 2 ;;
    --token)      TOKEN="$2";        shift 2 ;;
    --version)    VERSION="$2";      shift 2 ;;
    --no-service) NO_SERVICE=true;   shift   ;;
    --dir)        INSTALL_DIR="$2";  shift 2 ;;
    *) warn "Unknown flag: $1"; shift ;;
  esac
done

echo ""
echo -e "${BOLD}╔══════════════════════════════════════════════════════╗"
echo -e "║       Autonomyx Platform Installer                   ║"
echo -e "╚══════════════════════════════════════════════════════╝${NC}"
echo ""

# ── OS / arch detection ───────────────────────────────────────────────────────
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
[[ "$ARCH" == "aarch64" ]] && ARCH="aarch64"
[[ "$ARCH" == "arm64" ]]   && ARCH="aarch64"
[[ "$ARCH" == "x86_64" ]]  && ARCH="x86_64"

if [[ -f /etc/os-release ]]; then
  . /etc/os-release; DISTRO_ID="${ID:-unknown}"
else
  DISTRO_ID="unknown"
fi

SUDO=""
[[ $EUID -ne 0 ]] && SUDO="sudo"

# ── Helpers ───────────────────────────────────────────────────────────────────
install_docker_rhel() {
  $SUDO dnf -y install dnf-plugins-core
  $SUDO dnf config-manager --add-repo https://download.docker.com/linux/centos/docker-ce.repo
  $SUDO dnf -y install docker-ce docker-ce-cli containerd.io docker-compose-plugin
  $SUDO systemctl enable --now docker
}

install_docker_debian() {
  $SUDO apt-get update -qq
  $SUDO apt-get install -y -qq ca-certificates curl gnupg lsb-release
  $SUDO install -m 0755 -d /etc/apt/keyrings
  curl -fsSL "https://download.docker.com/linux/${DISTRO_ID}/gpg" \
    | $SUDO gpg --dearmor -o /etc/apt/keyrings/docker.gpg
  $SUDO chmod a+r /etc/apt/keyrings/docker.gpg
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
    https://download.docker.com/linux/${DISTRO_ID} $(lsb_release -cs) stable" \
    | $SUDO tee /etc/apt/sources.list.d/docker.list
  $SUDO apt-get update -qq
  $SUDO apt-get install -y -qq docker-ce docker-ce-cli containerd.io docker-compose-plugin
}

ensure_docker() {
  if command -v docker &>/dev/null; then
    success "Docker already installed: $(docker --version 2>&1 | head -1)"
    return
  fi
  info "Installing Docker CE..."
  case "$DISTRO_ID" in
    almalinux|rhel|rocky|centos|ol|fedora) install_docker_rhel ;;
    ubuntu|debian|linuxmint|pop)            install_docker_debian ;;
    *) die "Unsupported distro '$DISTRO_ID'. Install Docker manually: https://docs.docker.com/engine/install/" ;;
  esac
  if ! docker compose version &>/dev/null 2>&1; then
    die "docker compose plugin missing — install docker-compose-plugin and retry"
  fi
  success "Docker installed"
}

generate_token() {
  LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | head -c 64 2>/dev/null \
    || openssl rand -hex 32
}

# ── Mode: docker ──────────────────────────────────────────────────────────────
install_docker_mode() {
  ensure_docker
  $SUDO mkdir -p "$INSTALL_DIR"
  cd "$INSTALL_DIR"

  [[ -z "$TOKEN" ]] && TOKEN="$(generate_token)"

  # Prompt for LLM key if interactive
  LLM_API_KEY=""
  if [[ -t 0 ]]; then
    echo ""
    echo -e "${BOLD}API Keys (Enter to skip optional keys)${NC}"
    echo -e "  Default model: ${CYAN}${MODEL}${NC}"
    echo ""
    if [[ "$MODEL" == claude-* ]]; then
      read -rsp "  Anthropic API key: " LLM_API_KEY; echo
    elif [[ "$MODEL" == gpt-* ]]; then
      read -rsp "  OpenAI API key:    " LLM_API_KEY; echo
    fi
  fi

  # Write .env (never overwrite existing secrets)
  if [[ ! -f .env ]]; then
    cat > .env <<ENVEOF
# Autonomyx — generated $(date -u +"%Y-%m-%dT%H:%M:%SZ")
LLM_MODEL=${MODEL}
LLM_API_KEY=${LLM_API_KEY}
ANTHROPIC_API_KEY=${LLM_API_KEY}
OPENAI_API_KEY=
OLLAMA_BASE_URL=
BRAVE_API_KEY=
GATEWAY_API_KEY=${TOKEN}
PRODUCTION=true
AUTH_ENABLED=false
ALLOW_SHELL=false
PORT=${PORT_API}
RUST_LOG=agent_runner=info,tower_http=warn
ENVEOF
    $SUDO chmod 600 .env
    success "Wrote .env"
  else
    warn ".env already exists — not overwritten"
  fi

  # Write docker-compose.yml
  cat > docker-compose.yml <<COMPOSEEOF
# Autonomyx — upgrade: docker compose pull && docker compose up -d
services:
  agent-ide:
    image: ${IDE_IMAGE}
    restart: unless-stopped
    ports: ["${PORT_IDE}:3000"]
    env_file: .env
    volumes: [ide-data:/data]
    healthcheck:
      test: ["CMD", "wget", "-qO-", "http://localhost:3000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 30s

  agent-runner:
    image: ${RUNNER_IMAGE}
    restart: unless-stopped
    ports: ["${PORT_API}:3001"]
    env_file: .env
    healthcheck:
      test: ["CMD", "curl", "-sf", "http://localhost:3001/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 15s

volumes:
  ide-data:
COMPOSEEOF

  info "Pulling images..."
  $SUDO docker compose pull

  info "Starting platform..."
  $SUDO docker compose up -d

  if [[ "$NO_SERVICE" == "false" ]] && command -v systemctl &>/dev/null; then
    $SUDO tee /etc/systemd/system/autonomyx.service > /dev/null <<SVCEOF
[Unit]
Description=Autonomyx Platform
Requires=docker.service
After=docker.service network-online.target

[Service]
Type=oneshot
RemainAfterExit=yes
WorkingDirectory=${INSTALL_DIR}
ExecStart=docker compose up -d
ExecStop=docker compose down
TimeoutStartSec=120

[Install]
WantedBy=multi-user.target
SVCEOF
    $SUDO systemctl daemon-reload
    $SUDO systemctl enable autonomyx
    success "Systemd service enabled (auto-starts on reboot)"
  fi

  PUBLIC_IP="$(curl -sf https://api.ipify.org 2>/dev/null || echo "your-server-ip")"

  echo ""
  echo -e "${BOLD}${GREEN}╔══════════════════════════════════════════════════════╗"
  echo -e "║  Autonomyx is live!                                  ║"
  echo -e "╟──────────────────────────────────────────────────────╢"
  echo -e "║${NC}  IDE:    http://${PUBLIC_IP}"
  echo -e "${BOLD}${GREEN}║${NC}  API:    http://${PUBLIC_IP}:${PORT_API}"
  echo -e "${BOLD}${GREEN}║${NC}  Health: http://${PUBLIC_IP}:${PORT_API}/health"
  echo -e "${BOLD}${GREEN}║${NC}  Model:  ${MODEL}"
  echo -e "${BOLD}${GREEN}║${NC}"
  echo -e "${BOLD}${GREEN}║${NC}  Switch model live (no restart needed):"
  echo -e "${BOLD}${GREEN}║${NC}  curl -X PUT http://localhost:${PORT_API}/api/providers/config \\"
  echo -e "${BOLD}${GREEN}║${NC}       -d '{\"default_model\":\"gpt-4o\"}'"
  echo -e "${BOLD}${GREEN}║${NC}"
  echo -e "${BOLD}${GREEN}║${NC}  Upgrade:   cd ${INSTALL_DIR} && docker compose pull && docker compose up -d"
  echo -e "${BOLD}${GREEN}║${NC}  Logs:      docker compose -f ${INSTALL_DIR}/docker-compose.yml logs -f"
  echo -e "${BOLD}${GREEN}║${NC}  Stop:      systemctl stop autonomyx"
  echo -e "${BOLD}${GREEN}╚══════════════════════════════════════════════════════╝${NC}"
}

# ── Mode: binary ──────────────────────────────────────────────────────────────
install_binary_mode() {
  [[ "$OS" == "darwin" ]] && EXT="" || EXT=""
  TARBALL="${BINARY_NAME}-${OS}-${ARCH}.tar.gz"

  if [[ "$VERSION" == "latest" ]]; then
    URL="https://github.com/${GITHUB_REPO}/releases/latest/download/${TARBALL}"
  else
    URL="https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/${TARBALL}"
  fi

  # Allow local build fallback
  LOCAL="packages/agent-runner/target/release/${BINARY_NAME}"
  if [[ -f "$LOCAL" ]]; then
    info "Using local build: $LOCAL"
    BIN_SRC="$LOCAL"
  else
    info "Downloading ${TARBALL}..."
    TMP="$(mktemp -d)"; trap "rm -rf $TMP" EXIT
    curl -fsSL "$URL" -o "$TMP/$TARBALL"
    tar -xzf "$TMP/$TARBALL" -C "$TMP"
    BIN_SRC="$TMP/$BINARY_NAME"
  fi

  $SUDO install -m 755 "$BIN_SRC" "$BINARY_DIR/$BINARY_NAME"
  success "Installed: $BINARY_DIR/$BINARY_NAME"

  [[ -z "$TOKEN" ]] && TOKEN="$(generate_token)"
  CONFIG_DIR="/etc/autonomyx"
  $SUDO mkdir -p "$CONFIG_DIR"
  ENV_FILE="$CONFIG_DIR/autonomyx.env"

  if [[ ! -f "$ENV_FILE" ]]; then
    $SUDO tee "$ENV_FILE" > /dev/null <<ENVEOF
GATEWAY_API_KEY=${TOKEN}
LLM_MODEL=${MODEL}
LLM_API_KEY=
ANTHROPIC_API_KEY=
OPENAI_API_KEY=
OLLAMA_BASE_URL=
PORT=${PORT_API}
PRODUCTION=true
RUST_LOG=agent_runner=info,tower_http=warn
ENVEOF
    $SUDO chmod 600 "$ENV_FILE"
    success "Config: $ENV_FILE"
    warn "Edit $ENV_FILE to add your LLM API key"
  else
    warn "Config exists ($ENV_FILE) — not overwritten"
  fi

  if [[ "$NO_SERVICE" == "false" ]] && [[ "$OS" == "linux" ]] && command -v systemctl &>/dev/null; then
    $SUDO tee /etc/systemd/system/autonomyx.service > /dev/null <<SVCEOF
[Unit]
Description=Autonomyx Platform Runner
After=network.target
StartLimitIntervalSec=0

[Service]
Type=simple
User=nobody
EnvironmentFile=${ENV_FILE}
ExecStart=${BINARY_DIR}/${BINARY_NAME}
Restart=always
RestartSec=5
NoNewPrivileges=yes
PrivateTmp=yes

[Install]
WantedBy=multi-user.target
SVCEOF
    $SUDO systemctl daemon-reload
    $SUDO systemctl enable --now autonomyx
    success "Service started: systemctl status autonomyx"
  fi

  echo ""
  success "Binary install complete"
  echo "  Binary:  $BINARY_DIR/$BINARY_NAME"
  echo "  Config:  $ENV_FILE"
  echo "  Health:  curl http://localhost:${PORT_API}/health"
  echo "  API key: ${TOKEN}"
}

# ── Dispatch ──────────────────────────────────────────────────────────────────
case "$INSTALL_MODE" in
  docker) install_docker_mode ;;
  binary) install_binary_mode ;;
  *) die "Unknown mode: $INSTALL_MODE (use docker or binary)" ;;
esac
