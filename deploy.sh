#!/usr/bin/env bash
# Agent IDE — one-command k3s deploy
# Renders agent-compose.yml into k3s manifests and applies them.
#
# Usage:
#   ./deploy.sh                  # deploy :main
#   ./deploy.sh v1.2.3           # deploy specific tag
#   ./deploy.sh --profile ollama # include Ollama sidecar
#   ./deploy.sh --profile openhands
#   ./deploy.sh --profile ollama --profile openhands
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
NAMESPACE="agent-ide"
IMAGE_TAG="main"
PROFILES=()

# ── Parse args ────────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile) PROFILES+=("$2"); shift 2 ;;
    --namespace) NAMESPACE="$2"; shift 2 ;;
    -*) echo "Unknown flag: $1"; exit 1 ;;
    *) IMAGE_TAG="$1"; shift ;;
  esac
done

IMAGE="ghcr.io/agennext/agent-ide:${IMAGE_TAG}"

# ── 1. Ensure k3s ─────────────────────────────────────────────────────────────
if ! command -v k3s &>/dev/null; then
  echo "k3s not found — installing..."
  curl -sfL https://get.k3s.io | sh -
  sleep 5
fi

if ! command -v kubectl &>/dev/null; then
  export PATH="/usr/local/bin:$PATH"
  ln -sf "$(command -v k3s)" /usr/local/bin/kubectl 2>/dev/null || true
fi

if [[ -f /etc/rancher/k3s/k3s.yaml && -z "${KUBECONFIG:-}" ]]; then
  export KUBECONFIG=/etc/rancher/k3s/k3s.yaml
fi

echo "Deploying image: ${IMAGE}"
echo "Namespace:        ${NAMESPACE}"
[[ ${#PROFILES[@]} -gt 0 ]] && echo "Profiles:         ${PROFILES[*]}"

# ── 2. Render manifest from agent-compose.yml ─────────────────────────────────
# Use kompose if available, otherwise fall back to the static manifest
MANIFEST_DIR="$(mktemp -d)"
trap 'rm -rf "$MANIFEST_DIR"' EXIT

if command -v kompose &>/dev/null; then
  echo "Rendering via kompose..."
  PROFILE_FLAGS=()
  for p in "${PROFILES[@]}"; do PROFILE_FLAGS+=(--profile "$p"); done

  TAG="${IMAGE_TAG}" kompose convert \
    -f "${REPO_ROOT}/agent-compose.yml" \
    "${PROFILE_FLAGS[@]}" \
    --namespace "${NAMESPACE}" \
    --out "${MANIFEST_DIR}" 2>/dev/null

  # Patch image tag (kompose uses the compose image field verbatim)
  find "${MANIFEST_DIR}" -name "*.yaml" -exec \
    sed -i "s|ghcr.io/agennext/agent-ide:main|${IMAGE}|g" {} \;
else
  echo "kompose not found — using static manifest (deploy/k3s.yaml)"
  sed "s|ghcr.io/agennext/agent-ide:main|${IMAGE}|g" \
    "${REPO_ROOT}/deploy/k3s.yaml" > "${MANIFEST_DIR}/k3s.yaml"

  # Inject Ollama env if profile enabled
  if printf '%s\n' "${PROFILES[@]}" | grep -q "^ollama$"; then
    kubectl -n "${NAMESPACE}" create configmap agent-ide-ollama \
      --from-literal=OLLAMA_BASE_URL="http://ollama:11434/v1" \
      --dry-run=client -o yaml >> "${MANIFEST_DIR}/ollama-cm.yaml" || true
  fi
fi

# ── 3. Ensure namespace ────────────────────────────────────────────────────────
kubectl get namespace "${NAMESPACE}" &>/dev/null || \
  kubectl create namespace "${NAMESPACE}"

# ── 4. Apply ──────────────────────────────────────────────────────────────────
kubectl apply -n "${NAMESPACE}" -f "${MANIFEST_DIR}/"

# ── 5. Wait ───────────────────────────────────────────────────────────────────
echo "Waiting for rollout..."
kubectl rollout status deployment/agent-ide -n "${NAMESPACE}" --timeout=120s

# ── 6. Print info ─────────────────────────────────────────────────────────────
NODE_IP=$(kubectl get nodes \
  -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}' \
  2>/dev/null || echo "127.0.0.1")

echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║  Agent IDE is running!                               ║"
echo "╟──────────────────────────────────────────────────────╢"
echo "║  URL:   http://${NODE_IP}                            ║"
echo "║                                                      ║"
echo "║  Add API keys:                                       ║"
echo "║    kubectl -n ${NAMESPACE} create secret generic \\  ║"
echo "║      agent-ide-secrets \\                             ║"
echo "║      --from-env-file=.env \\                          ║"
echo "║      --dry-run=client -o yaml | kubectl apply -f -   ║"
echo "║                                                      ║"
echo "║  Tear down:                                          ║"
echo "║    kubectl delete namespace ${NAMESPACE}             ║"
echo "╚══════════════════════════════════════════════════════╝"
