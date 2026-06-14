#!/bin/bash
# Agent-IDE native install for macOS (Apple Silicon + Intel)
# Installs agent-runner binary + Theia IDE as a launchd service
set -euo pipefail

REPO="AGenNext/Agent-IDE"
INSTALL_DIR="/usr/local/lib/agent-ide"
BIN_DIR="/usr/local/bin"
PLIST_DIR="$HOME/Library/LaunchAgents"
PLIST_ID="com.agennext.agent-ide"

# ── Detect arch ──────────────────────────────────────────────────────────────
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    ARTIFACT="agent-runner-darwin-arm64"
else
    ARTIFACT="agent-runner-darwin-amd64"
fi

echo "▶ Agent-IDE installer for macOS ($ARCH)"

# ── Fetch latest release ─────────────────────────────────────────────────────
TAG=${1:-$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)}
echo "  Release: $TAG"

BINARY_URL="https://github.com/$REPO/releases/download/$TAG/$ARTIFACT"
echo "  Downloading: $BINARY_URL"

sudo mkdir -p "$INSTALL_DIR"
sudo curl -fsSL "$BINARY_URL" -o "$INSTALL_DIR/agent-runner"
sudo chmod +x "$INSTALL_DIR/agent-runner"
sudo ln -sf "$INSTALL_DIR/agent-runner" "$BIN_DIR/agent-runner"

# ── launchd plist ────────────────────────────────────────────────────────────
mkdir -p "$PLIST_DIR"
cat > "$PLIST_DIR/$PLIST_ID.backend.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>        <string>$PLIST_ID.backend</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_DIR/agent-runner</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PORT</key>           <string>3001</string>
        <key>RUNTIME_PHASE</key>  <string>2</string>
        <key>WORKSPACE_ROOT</key> <string>$HOME/.agent-ide/workspace</string>
    </dict>
    <key>WorkingDirectory</key> <string>$INSTALL_DIR</string>
    <key>RunAtLoad</key>        <true/>
    <key>KeepAlive</key>        <true/>
    <key>StandardOutPath</key>  <string>$HOME/.agent-ide/backend.log</string>
    <key>StandardErrorPath</key><string>$HOME/.agent-ide/backend.err</string>
</dict>
</plist>
EOF

mkdir -p "$HOME/.agent-ide/workspace"

launchctl unload "$PLIST_DIR/$PLIST_ID.backend.plist" 2>/dev/null || true
launchctl load   "$PLIST_DIR/$PLIST_ID.backend.plist"

echo ""
echo "✓ agent-runner installed and started (Phase 2 Rust backend)"
echo "  Backend:   http://localhost:3001/health"
echo "  Logs:      $HOME/.agent-ide/backend.log"
echo ""
echo "  To open the IDE in browser:"
echo "  open http://localhost:3000"
echo ""
echo "  To uninstall: launchctl unload $PLIST_DIR/$PLIST_ID.backend.plist"
