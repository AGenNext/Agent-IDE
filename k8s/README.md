# Agent IDE — Kubernetes Deployment

## Helm deploy

```bash
# Add secrets (do not commit real values)
helm install agent-ide ./helm/agent-ide \
  --namespace agent-ide --create-namespace \
  --set env.anthropicApiKey=sk-ant-... \
  --set env.openaiApiKey=sk-... \
  --set env.jwtSecret=$(openssl rand -hex 32) \
  --set ingress.enabled=true \
  --set ingress.host=agent-ide.example.com

# Upgrade
helm upgrade agent-ide ./helm/agent-ide --namespace agent-ide --reuse-values
```

## Kustomize deploy

```bash
# Base (adjust namespace/secrets first)
kubectl apply -k k8s/base

# Dev overlay
kubectl apply -k k8s/overlays/dev

# Prod overlay
kubectl apply -k k8s/overlays/prod

# GPU overlay (Ollama / vLLM clusters)
kubectl apply -k k8s/overlays/gpu
```

## Multi-cluster deploy

Use `--kubeconfig` or the `KUBECONFIG` env var to target different clusters:

```bash
# Helm — target a specific cluster
helm install agent-ide ./helm/agent-ide \
  --kubeconfig ~/.kube/prod-cluster.yaml \
  --namespace agent-ide --create-namespace \
  -f helm/agent-ide/values-prod.yaml

# Kustomize — target a specific cluster
KUBECONFIG=~/.kube/prod-cluster.yaml kubectl apply -k k8s/overlays/prod
```

## Per-cluster values (Helm)

Create a values override file per cluster:

```bash
# helm/agent-ide/values-prod.yaml
backend:
  replicaCount: 2
ingress:
  enabled: true
  host: agent-ide.prod.example.com
  tls:
    enabled: true
    secretName: agent-ide-tls

helm upgrade --install agent-ide ./helm/agent-ide \
  --namespace agent-ide --create-namespace \
  -f helm/agent-ide/values.yaml \
  -f helm/agent-ide/values-prod.yaml \
  --set env.anthropicApiKey=$ANTHROPIC_API_KEY
```
