# Deploy Agent IDE on k3s

This runbook deploys Agent IDE to an existing k3s cluster.

## Preconditions

- `kubectl` is installed and can reach the cluster.
- k3s `local-path` storage class exists.
- Container runtime can pull `ghcr.io/agennext/agent-ide:<tag>`.
- The image tag has already been built and pushed.
- Optional API keys are available in `.env` before running deploy.

## Build and push image

From the repo root:

```bash
export TAG=$(git rev-parse --short HEAD)
yarn install --frozen-lockfile
yarn build
docker build -f Containerfile -t ghcr.io/agennext/agent-ide:$TAG .
docker push ghcr.io/agennext/agent-ide:$TAG
```

## Optional secrets

Create `.env` on the deploy host. Do not commit this file.

```bash
cat > .env <<'EOF'
OPENAI_API_KEY=
ANTHROPIC_API_KEY=
BRAVE_API_KEY=
EOF
```

The deploy script will apply this into the `agent-ide-secrets` Kubernetes Secret if `.env` exists.

## Deploy

```bash
./deploy.sh $TAG --namespace agent-ide
```

With local Ollama profile metadata:

```bash
./deploy.sh $TAG --namespace agent-ide --profile ollama
```

## Verify

```bash
kubectl -n agent-ide get pods,svc,ingress,pvc
kubectl -n agent-ide rollout status deployment/agent-ide
kubectl -n agent-ide logs deploy/agent-ide --tail=100
```

Port-forward locally:

```bash
kubectl -n agent-ide port-forward svc/agent-ide-ide 3000:3000
kubectl -n agent-ide port-forward svc/agent-ide-backend 3001:3001
```

Then open:

- IDE: `http://127.0.0.1:3000`
- Backend health: `http://127.0.0.1:3001/health`

## Rollback

```bash
kubectl -n agent-ide rollout undo deployment/agent-ide
kubectl -n agent-ide rollout status deployment/agent-ide
```

## Remove

```bash
kubectl delete namespace agent-ide
```

## Production notes

- `ALLOW_SHELL=false` by default.
- MCP execution is policy-gated and approval-gated.
- The Deployment mounts `/data` through a PVC.
- Ingress assumes k3s Traefik.
- For public exposure, add a DNS host and TLS/cert-manager layer before production traffic.
