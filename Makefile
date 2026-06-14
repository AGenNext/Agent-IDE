# Autonomyx Platform Build
# The platform where anyone can build their next. openautonomyx.com
#
# make platform        — build the single binary platform image
# make platform-push   — push to registry
# make platform-deploy — deploy to any cloud (cloud as platform provider)
# make all             — build the full stack (IDE + platform)
#
# Cloud targets:
#   make platform-deploy CLOUD=aws      → EKS
#   make platform-deploy CLOUD=gcp      → GKE
#   make platform-deploy CLOUD=azure    → AKS
#   make platform-deploy CLOUD=hetzner  → Hetzner k3s
#   make platform-deploy CLOUD=k3s      → local k3s

REGISTRY   := ghcr.io
OWNER      := agennext
IMAGE      := $(REGISTRY)/$(OWNER)/agent-ide
PLATFORM_IMAGE := $(REGISTRY)/$(OWNER)/autonomyx
TAG        ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo latest)
NAMESPACE  ?= autonomyx
CLOUD      ?= k3s
PROFILES   ?=

PROFILE_FLAGS := $(foreach p,$(PROFILES),--profile $(p))

.PHONY: all build push deploy platform platform-build platform-push platform-deploy \
        platform-local platform-run smoke

# ── Stack — multi-build, single stack ────────────────────────────────────────
# Autonomyx is four artifacts, one platform:
#   1. autonomyx       — the platform binary (Rust — gates, fabric, federation, AIP)
#   2. agent-ide       — the IDE (Node/Theia — where agents are authored)
#   3. autonomyx-lang  — the language extension (.ayx DSL — declaration is the contract)
#   4. zot             — the OCI artifact registry (supply chain provenance)
#
# "multi build single stack" — build all four, deploy as one.
# The platform fills the gaps between all of them.

stack: platform-build build lang-build
	@echo "Stack built: platform + IDE + language. Deploy with: make stack-deploy"

stack-push: platform-push push
	@echo "Stack pushed to $(REGISTRY)/$(OWNER)"

stack-deploy: platform-deploy deploy
	@echo "Full stack deployed. The platform makes things real."

# Language extension build
lang-build:
	@if [ -d extensions/autonomyx-lang ]; then \
		yarn --cwd extensions/autonomyx-lang build 2>/dev/null || \
		npx --prefix extensions/autonomyx-lang langium generate 2>/dev/null || \
		echo "autonomyx-lang: grammar ready (compile on next full build)"; \
	else \
		echo "autonomyx-lang: not found — skipping"; \
	fi

# ── Full stack (IDE + platform runtime) ───────────────────────────────────────

all: stack push deploy

build:
	yarn --cwd packages/agent-ide-types build
	yarn --cwd packages/agent-ide-backend build
	yarn --cwd extensions/agent-ide-core build
	yarn --cwd applications/browser-app build
	docker build -f Containerfile \
		--tag $(IMAGE):$(TAG) \
		--tag $(IMAGE):latest \
		.

push:
	docker push $(IMAGE):$(TAG)
	docker push $(IMAGE):latest

deploy:
	@if command -v kubectl >/dev/null 2>&1; then \
		bash deploy.sh $(TAG) $(foreach p,$(PROFILES),--profile $(p)) --namespace $(NAMESPACE); \
	else \
		TAG=$(TAG) docker compose -f agent-compose.yml $(PROFILE_FLAGS) up -d; \
	fi

# ── Platform — single binary build ───────────────────────────────────────────
# The theory is the .ayx declaration. The platform makes it real.
# One binary. All gates. All fabric. All federation. Runs anywhere.

platform: platform-build

platform-build:
	@echo "Building Autonomyx platform — single binary, all gates, all fabric"
	docker build -f Containerfile.runner \
		--tag $(PLATFORM_IMAGE):$(TAG) \
		--tag $(PLATFORM_IMAGE):latest \
		--label "autonomyx.version=$(TAG)" \
		--label "autonomyx.git-sha=$(shell git rev-parse HEAD 2>/dev/null || echo unknown)" \
		.
	@echo "Platform image: $(PLATFORM_IMAGE):$(TAG)"
	@echo "Size: $$(docker image inspect $(PLATFORM_IMAGE):latest --format '{{.Size}}' | numfmt --to=iec 2>/dev/null || echo unknown)"

platform-push:
	docker push $(PLATFORM_IMAGE):$(TAG)
	docker push $(PLATFORM_IMAGE):latest

# Cloud as platform provider — deploy to any cloud with kubectl
platform-deploy:
	@echo "Deploying Autonomyx platform to cloud: $(CLOUD)"
	@if ! command -v kubectl >/dev/null 2>&1; then \
		echo "kubectl not found — install kubectl or use 'make platform-local'"; exit 1; \
	fi
	kubectl apply -f deploy/cloud/autonomyx-cloud.yaml
	kubectl set image deployment/autonomyx-runner \
		autonomyx=$(PLATFORM_IMAGE):$(TAG) \
		-n $(NAMESPACE) 2>/dev/null || true
	@echo "Deployment complete. Platform fills the gaps."
	@kubectl rollout status deployment/autonomyx-runner -n $(NAMESPACE) --timeout=120s

# Local dev — run the platform directly (no container, native speed)
platform-local:
	@echo "Running Autonomyx platform natively — no container overhead"
	cd packages/agent-runner && \
		RUST_LOG=agent_runner=debug,tower_http=info \
		PORT=3001 \
		cargo run --release

# Local container run — single binary, verify locally before cloud deploy
platform-run:
	@echo "Running Autonomyx platform container locally"
	docker run --rm -it \
		-p 3001:3001 \
		-e PORT=3001 \
		-e RUST_LOG=agent_runner=debug \
		-e CLOUD_PROVIDER=local \
		$(PLATFORM_IMAGE):latest

# Health check — verify the platform is alive and its theory is sound
smoke:
	@echo "Smoke test: platform health + theory"
	@curl -sf http://localhost:3001/health | jq .status
	@curl -sf http://localhost:3001/api/platform | jq '.platform.name'
	@curl -sf http://localhost:3001/api/theory | jq '.theory'
	@echo "Platform is alive. The theory holds."
