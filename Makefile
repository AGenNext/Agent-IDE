REGISTRY   := ghcr.io
OWNER      := agennext
IMAGE      := $(REGISTRY)/$(OWNER)/agent-ide
TAG        ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo latest)
NAMESPACE  ?= agent-ide
PROFILES   ?=

PROFILE_FLAGS := $(foreach p,$(PROFILES),--profile $(p))

# ── Pipeline ──────────────────────────────────────────────────────────────────
.PHONY: all build-code build-container push-image deploy-app

all: build-code build-container push-image deploy-app   ## run full pipeline

build-code:        ## compile TypeScript (all packages)
	@echo "▶  build-code..."
	yarn --cwd packages/agent-ide-types build
	yarn --cwd packages/agent-ide-backend build
	yarn --cwd extensions/agent-ide-core build
	yarn --cwd applications/browser-app build
	@echo "✔  done"

build-container:   ## docker build → :SHA + :latest
	@docker compose -f agent-compose.yml $(PROFILE_FLAGS) config --quiet
	@echo "▶  build-container $(IMAGE):$(TAG)..."
	docker build -f Containerfile \
		--tag $(IMAGE):$(TAG) \
		--tag $(IMAGE):latest \
		--label "org.opencontainers.image.revision=$(shell git rev-parse HEAD 2>/dev/null)" \
		--label "org.opencontainers.image.created=$(shell date -u +%Y-%m-%dT%H:%M:%SZ)" \
		.
	@echo "✔  $(IMAGE):$(TAG)"

push-image:        ## push image to GHCR
	@echo "▶  push-image $(IMAGE):$(TAG)..."
	docker push $(IMAGE):$(TAG)
	docker push $(IMAGE):latest
	@echo "✔  pushed"

deploy-app:        ## deploy app (k3s or docker compose)
	@if command -v kubectl >/dev/null 2>&1; then \
		bash deploy.sh $(TAG) $(foreach p,$(PROFILES),--profile $(p)) --namespace $(NAMESPACE); \
	else \
		TAG=$(TAG) docker compose -f agent-compose.yml $(PROFILE_FLAGS) up -d; \
		echo "✔  http://localhost"; \
	fi

# ── Tools ─────────────────────────────────────────────────────────────────────
.PHONY: tail-logs stop-app help

tail-logs:         ## stream app logs
	@if command -v kubectl >/dev/null 2>&1; then \
		kubectl logs -n $(NAMESPACE) -l app=agent-ide -f; \
	else \
		docker compose -f agent-compose.yml logs -f; \
	fi

stop-app:          ## stop and remove the app
	@if command -v kubectl >/dev/null 2>&1; then \
		kubectl delete namespace $(NAMESPACE) --ignore-not-found; \
	else \
		docker compose -f agent-compose.yml down; \
	fi

help:
	@echo "Pipeline:"; grep -E '^[a-z].*:.*## ' Makefile | head -5 | sed 's/:.*## /\t/'
	@echo ""; echo "Tools:"; grep -E '^[a-z].*:.*## ' Makefile | tail -3 | sed 's/:.*## /\t/'
