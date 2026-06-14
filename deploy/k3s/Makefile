# Autonomyx on k3s — make targets
# Kubernetes IS the agent runner.
#
# Prerequisites:
#   k3s installed: curl -sfL https://get.k3s.io | sh -
#   kubectl: k3s kubectl (or export KUBECONFIG=/etc/rancher/k3s/k3s.yaml)
#   cargo (Rust toolchain) for building the runner binary
#
# Autonomyx is the platform. openautonomyx.com

NAMESPACE      := autonomyx
IMAGE          := autonomyx-runner:dev
RUNNER_BIN     := ../../packages/agent-runner
K3S_KUBECONFIG := /etc/rancher/k3s/k3s.yaml

KUBECTL := kubectl --kubeconfig=$(K3S_KUBECONFIG)

.PHONY: all build load deploy status logs port-forward clean

all: build load deploy

## Build the Rust binary and package it into an OCI image
build:
	@echo "==> Building autonomyx-runner (Rust, musl static)"
	cargo build --release --manifest-path=$(RUNNER_BIN)/Cargo.toml
	@echo "==> Packaging OCI image"
	mkdir -p /tmp/autonomyx-image
	cp $(RUNNER_BIN)/target/release/autonomyx-runner /tmp/autonomyx-image/
	cat > /tmp/autonomyx-image/Dockerfile <<'EOF'
	FROM scratch
	COPY autonomyx-runner /autonomyx-runner
	USER 1001
	EXPOSE 3001
	ENTRYPOINT ["/autonomyx-runner"]
	EOF
	docker build -t $(IMAGE) /tmp/autonomyx-image

## Load image into k3s (no external registry needed on local k3s)
load:
	@echo "==> Loading $(IMAGE) into k3s containerd"
	docker save $(IMAGE) | sudo k3s ctr images import -

## Deploy to k3s
deploy:
	@echo "==> Deploying Autonomyx to k3s namespace: $(NAMESPACE)"
	$(KUBECTL) apply -f autonomyx-k3s.yaml
	$(KUBECTL) rollout restart deployment/autonomyx-runner -n $(NAMESPACE)
	$(KUBECTL) rollout status  deployment/autonomyx-runner -n $(NAMESPACE)

## Show status: pods, jobs, gates
status:
	@echo "==> Pods"
	$(KUBECTL) get pods -n $(NAMESPACE) -o wide
	@echo ""
	@echo "==> Jobs (agent runs)"
	$(KUBECTL) get jobs -n $(NAMESPACE)
	@echo ""
	@echo "==> Services"
	$(KUBECTL) get svc -n $(NAMESPACE)

## Tail runner logs
logs:
	$(KUBECTL) logs -n $(NAMESPACE) -l app=autonomyx-runner --tail=100 -f

## Local access to the platform API
port-forward:
	@echo "==> Forwarding localhost:3001 → autonomyx-runner:3001"
	@echo "    Health: http://localhost:3001/health"
	@echo "    Gates:  POST http://localhost:3001/api/lifecycle/transition"
	@echo "    Fabric: GET  http://localhost:3001/api/fabric/log"
	$(KUBECTL) port-forward -n $(NAMESPACE) svc/autonomyx-runner 3001:3001

## Quick smoke test via port-forward (run in background first)
smoke:
	@echo "==> Health check"
	curl -s http://localhost:3001/health | python3 -m json.tool
	@echo ""
	@echo "==> Kick a build gate (idempotent — run twice to verify)"
	curl -s -X POST http://localhost:3001/api/lifecycle/transition \
	  -H "Authorization: Bearer $$GATEWAY_API_KEY" \
	  -H "Content-Type: application/json" \
	  -d '{"artifact":"test-image@sha256:abc","stage":"build","payload":{"digest":"sha256:abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890a","platform":"linux/amd64"}}' \
	  | python3 -m json.tool
	@echo ""
	@echo "==> Fabric event log"
	curl -s http://localhost:3001/api/fabric/log | python3 -m json.tool

## Remove all Autonomyx resources from k3s
clean:
	$(KUBECTL) delete namespace $(NAMESPACE) --ignore-not-found
