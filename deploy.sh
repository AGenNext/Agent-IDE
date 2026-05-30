#!/usr/bin/env bash
# Agent IDE — one-command k3s deploy
# Usage:  ./deploy.sh [image-tag]
# Example: ./deploy.sh v1.2.3
#          ./deploy.sh  (defaults to :main)
set -euo pipefail

IMAGE_TAG="${1:-main}"
IMAGE="ghcr.io/agennext/agent-ide:${IMAGE_TAG}"
MANIFEST="$(dirname "$0")/deploy/k3s.yaml"

# ── 1. Ensure k3s is installed ────────────────────────────────────────────────
if ! command -v k3s &>/dev/null; then
  echo "k3s not found — installing..."
  curl -sfL https://get.k3s.io | sh -
  # Give k3s a moment to start
  sleep 5
fi

# ── 2. Ensure kubectl alias works ────────────────────────────────────────────
if ! command -v kubectl &>/dev/null; then
  # k3s ships its own kubectl via symlink; expose it
  export PATH="/usr/local/bin:$PATH"
  if ! command -v kubectl &>/dev/null; then
    ln -sf "$(command -v k3s)" /usr/local/bin/kubectl || true
  fi
fi

# Use k3s kubeconfig when running as root on the same node
if [[ -f /etc/rancher/k3s/k3s.yaml && -z "${KUBECONFIG:-}" ]]; then
  export KUBECONFIG=/etc/rancher/k3s/k3s.yaml
fi

echo "Using image: ${IMAGE}"

# ── 3. Apply namespace + base resources ──────────────────────────────────────
# Patch image tag before applying
kubectl apply -f <(sed "s|ghcr.io/agennext/agent-ide:main|${IMAGE}|g" "${MANIFEST}")

# ── 4. Wait for rollout ───────────────────────────────────────────────────────
echo "Waiting for rollout…"
kubectl rollout status deployment/agent-ide -n agent-ide --timeout=120s

# ── 5. Print access info ─────────────────────────────────────────────────────
NODE_IP=$(kubectl get nodes -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}' 2>/dev/null || echo "127.0.0.1")

echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║  Agent IDE is running!                               ║"
echo "╟──────────────────────────────────────────────────────╢"
echo "║  URL:  http://${NODE_IP}                             ║"
echo "║                                                      ║"
echo "║  Add API keys (optional):                            ║"
echo "║    kubectl -n agent-ide create secret generic \\      ║"
echo "║      agent-ide-secrets \\                             ║"
echo "║      --from-literal=ANTHROPIC_API_KEY=sk-ant-... \\   ║"
echo "║      --from-env-file=.env \\                          ║"
echo "║      --dry-run=client -o yaml | kubectl apply -f -   ║"
echo "║                                                      ║"
echo "║  Tear down:                                          ║"
echo "║    kubectl delete namespace agent-ide                ║"
echo "╚══════════════════════════════════════════════════════╝"
