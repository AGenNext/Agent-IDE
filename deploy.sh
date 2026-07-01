#!/usr/bin/env bash
# Agent IDE — production-safe k3s deploy
#
# This script deploys an already-built Agent IDE image to an existing k3s cluster.
# It intentionally does NOT install k3s or mutate the host outside Kubernetes.
#
# Usage:
#   ./deploy.sh main
#   ./deploy.sh v1.2.3 --namespace agent-ide
#   ./deploy.sh 807846c --profile ollama --namespace agent-ide
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
NAMESPACE="agent-ide"
IMAGE_TAG="main"
PROFILES=()
TIMEOUT="180s"

# ── Parse args ────────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile) PROFILES+=("$2"); shift 2 ;;
    --namespace) NAMESPACE="$2"; shift 2 ;;
    --timeout) TIMEOUT="$2"; shift 2 ;;
    -*) echo "Unknown flag: $1"; exit 1 ;;
    *) IMAGE_TAG="$1"; shift ;;
  esac
done

IMAGE="ghcr.io/agennext/agent-ide:${IMAGE_TAG}"

# ── 1. Preconditions ─────────────────────────────────────────────────────────
if ! command -v kubectl &>/dev/null; then
  echo "kubectl is required. Install kubectl or run on a host where k3s exposes kubectl."
  exit 1
fi

if ! kubectl version --client=true &>/dev/null; then
  echo "kubectl is installed but not usable. Check PATH and kubeconfig."
  exit 1
fi

if ! kubectl cluster-info &>/dev/null; then
  echo "No reachable Kubernetes cluster. Set KUBECONFIG or run on the k3s host."
  exit 1
fi

if ! kubectl get storageclass local-path &>/dev/null; then
  echo "StorageClass local-path not found. k3s local-path storage is required for this manifest."
  exit 1
fi

if [[ ! -f "${REPO_ROOT}/deploy/k3s.yaml" ]]; then
  echo "Missing deploy/k3s.yaml"
  exit 1
fi

cat <<INFO
Deploying Agent IDE
  image:      ${IMAGE}
  namespace:  ${NAMESPACE}
  timeout:    ${TIMEOUT}
INFO
[[ ${#PROFILES[@]} -gt 0 ]] && echo "  profiles:   ${PROFILES[*]}"

# ── 2. Render manifest ───────────────────────────────────────────────────────
MANIFEST_DIR="$(mktemp -d)"
trap 'rm -rf "$MANIFEST_DIR"' EXIT

sed "s|ghcr.io/agennext/agent-ide:main|${IMAGE}|g" \
  "${REPO_ROOT}/deploy/k3s.yaml" > "${MANIFEST_DIR}/k3s.yaml"

# Optional local model profile config.
if printf '%s\n' "${PROFILES[@]}" | grep -q "^ollama$"; then
  cat > "${MANIFEST_DIR}/ollama-config.yaml" <<YAML
apiVersion: v1
kind: ConfigMap
metadata:
  name: agent-ide-ollama
  namespace: ${NAMESPACE}
data:
  OLLAMA_BASE_URL: "http://ollama:11434/v1"
YAML
fi

# ── 3. Namespace and secrets ─────────────────────────────────────────────────
kubectl get namespace "${NAMESPACE}" &>/dev/null || kubectl create namespace "${NAMESPACE}"

if [[ -f "${REPO_ROOT}/.env" ]]; then
  kubectl -n "${NAMESPACE}" create secret generic agent-ide-secrets \
    --from-env-file="${REPO_ROOT}/.env" \
    --dry-run=client -o yaml | kubectl apply -f -
else
  echo "No .env found. Deploying with existing or placeholder agent-ide-secrets."
fi

# ── 4. Apply and wait ────────────────────────────────────────────────────────
kubectl apply -n "${NAMESPACE}" -f "${MANIFEST_DIR}/"

kubectl rollout status deployment/agent-ide -n "${NAMESPACE}" --timeout="${TIMEOUT}"

# ── 5. Verify health from inside the cluster ─────────────────────────────────
kubectl -n "${NAMESPACE}" run agent-ide-healthcheck \
  --rm -i --restart=Never \
  --image=curlimages/curl:8.10.1 \
  --command -- curl -fsS "http://agent-ide-backend:3001/health"

# ── 6. Print access information ──────────────────────────────────────────────
NODE_IP=$(kubectl get nodes \
  -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}' \
  2>/dev/null || echo "127.0.0.1")

cat <<INFO

Agent IDE deployment complete.

Cluster URL, if ingress is exposed:
  http://${NODE_IP}/

Local port-forward:
  kubectl -n ${NAMESPACE} port-forward svc/agent-ide-ide 3000:3000
  kubectl -n ${NAMESPACE} port-forward svc/agent-ide-backend 3001:3001

Health check:
  curl http://127.0.0.1:3001/health

Status:
  kubectl -n ${NAMESPACE} get pods,svc,ingress,pvc

Rollback:
  kubectl -n ${NAMESPACE} rollout undo deployment/agent-ide
INFO
