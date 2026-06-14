#!/usr/bin/env bash
# VPS deploy — OCI (Oracle Cloud) + Cloud Native Buildpacks + Podman + Caddy
#
# Build side (CI / local):
#   pack build ghcr.io/ORG/APP:TAG --builder paketobuildpacks/builder:base
#   docker push ghcr.io/ORG/APP:TAG
#
# Deploy side (this script, run on the OCI VPS):
#   ./vps-deploy.sh <image> <domain> [port]
#   Example: ./vps-deploy.sh ghcr.io/unboxd-cloud/arithmetic-platform:main arithmetic.agennext.com 3000
set -euo pipefail

IMAGE="${1:?Usage: ./vps-deploy.sh <image> <domain> [port]}"
DOMAIN="${2:?}"
PORT="${3:-3000}"
APP="$(echo "$IMAGE" | awk -F'[/:]' '{print $(NF-1)}' | tr '[:upper:]' '[:lower:]')"
SERVICE="${APP}"

echo "▶  image:   $IMAGE"
echo "▶  domain:  $DOMAIN"
echo "▶  port:    $PORT"
echo "▶  service: $SERVICE"

# ── 1. OCI firewall ──────────────────────────────────────────────────────────
# Open 80 + 443 in iptables (OCI Ubuntu instances block these by default).
# Also open them in OCI Console → VCN → Security List → Ingress Rules.
iptables  -C INPUT -p tcp --dport 80  -j ACCEPT 2>/dev/null || iptables  -I INPUT 6 -p tcp --dport 80  -j ACCEPT
iptables  -C INPUT -p tcp --dport 443 -j ACCEPT 2>/dev/null || iptables  -I INPUT 6 -p tcp --dport 443 -j ACCEPT
netfilter-persistent save 2>/dev/null || true

# ── 2. Podman (OCI-native container runtime, no daemon) ──────────────────────
if ! command -v podman &>/dev/null; then
  apt-get update && apt-get install -y podman
fi

# ── 3. Caddy ─────────────────────────────────────────────────────────────────
if ! command -v caddy &>/dev/null; then
  apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
    | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
    | tee /etc/apt/sources.list.d/caddy-stable.list
  apt-get update && apt-get install -y caddy
fi

# ── 4. GHCR auth (if private image) ─────────────────────────────────────────
if [[ -n "${GHCR_TOKEN:-}" ]]; then
  echo "$GHCR_TOKEN" | podman login ghcr.io -u "${GHCR_USER:-}" --password-stdin
fi

# ── 5. Pull latest image ─────────────────────────────────────────────────────
podman pull "$IMAGE"

# ── 6. .env ──────────────────────────────────────────────────────────────────
ENV_FILE="/etc/${SERVICE}.env"
if [[ ! -f "$ENV_FILE" ]]; then
  printf "NODE_ENV=production\nPORT=%s\n" "$PORT" > "$ENV_FILE"
  chmod 600 "$ENV_FILE"
  echo "⚠  Created $ENV_FILE — add secrets before starting"
fi

# ── 7. Podman systemd service (quadlet) ──────────────────────────────────────
mkdir -p /etc/containers/systemd
cat > /etc/containers/systemd/${SERVICE}.container <<EOF
[Unit]
Description=${APP}
After=network-online.target

[Container]
Image=${IMAGE}
EnvironmentFile=${ENV_FILE}
PublishPort=127.0.0.1:${PORT}:${PORT}
Volume=/var/lib/${SERVICE}:/data
AutoUpdate=registry

[Service]
Restart=on-failure

[Install]
WantedBy=multi-user.target default.target
EOF

mkdir -p "/var/lib/${SERVICE}"
systemctl daemon-reload
systemctl enable --now "$(systemctl list-unit-files | grep "${SERVICE}" | awk '{print $1}' | head -1 || echo "${SERVICE}")" 2>/dev/null || \
  systemctl restart "${SERVICE}" 2>/dev/null || true

# ── 8. Caddy ─────────────────────────────────────────────────────────────────
cat > /etc/caddy/Caddyfile <<EOF
${DOMAIN} {
    reverse_proxy localhost:${PORT}
}
EOF
systemctl restart caddy

# ── Done ─────────────────────────────────────────────────────────────────────
echo ""
echo "✔  ${APP} → https://${DOMAIN}"
echo ""
echo "   REMINDER: Open ports 80 + 443 in OCI Console:"
echo "   Networking → VCN → Security Lists → Ingress Rules"
echo ""
echo "   Logs:       journalctl -u ${SERVICE} -f"
echo "   Restart:    systemctl restart ${SERVICE}"
echo "   Auto-update: systemctl enable --now podman-auto-update.timer"
