REGISTRY   := ghcr.io
OWNER      := agennext
IMAGE      := $(REGISTRY)/$(OWNER)/agent-ide
TAG        ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo latest)
NAMESPACE  ?= agent-ide
PROFILES   ?=

PROFILE_FLAGS := $(foreach p,$(PROFILES),--profile $(p))

.PHONY: all build-code build-container push-image deploy-cluster \
        validate-compose tail-logs stop-cluster help

all: validate-compose build-code build-container push-image deploy-cluster

validate-compose:            ## validate agent-compose.yml
	@echo "▶  validate-compose..."
	docker compose -f agent-compose.yml $(PROFILE_FLAGS) config --quiet
	@echo "✔  OK"

build-code:                  ## compile TypeScript (all packages)
	@echo "▶  build-code..."
	yarn --cwd packages/agent-ide-types build
	yarn --cwd packages/agent-ide-backend build
	yarn --cwd extensions/agent-ide-core build
	yarn --cwd applications/browser-app build
	@echo "✔  build-code done"

build-container:             ## docker build → :SHA + :latest
	@echo "▶  build-container $(IMAGE):$(TAG)..."
	docker build -f Containerfile \
		--tag $(IMAGE):$(TAG) \
		--tag $(IMAGE):latest \
		--label "org.opencontainers.image.revision=$(shell git rev-parse HEAD 2>/dev/null)" \
		--label "org.opencontainers.image.created=$(shell date -u +%Y-%m-%dT%H:%M:%SZ)" \
		.
	@echo "✔  $(IMAGE):$(TAG)"

push-image:                  ## push image to GHCR
	@echo "▶  push-image $(IMAGE):$(TAG)..."
	docker push $(IMAGE):$(TAG)
	docker push $(IMAGE):latest
	@echo "✔  pushed"

deploy-cluster:              ## deploy to k3s (falls back to docker compose)
	@if command -v k3s >/dev/null 2>&1 || command -v kubectl >/dev/null 2>&1; then \
		echo "▶  deploy-cluster → k3s ($(NAMESPACE))"; \
		bash deploy.sh $(TAG) $(foreach p,$(PROFILES),--profile $(p)) --namespace $(NAMESPACE); \
	else \
		echo "▶  deploy-cluster → docker compose"; \
		TAG=$(TAG) docker compose -f agent-compose.yml $(PROFILE_FLAGS) up -d; \
		echo "✔  http://localhost"; \
	fi

tail-logs:                   ## stream logs (k3s or docker)
	@if command -v kubectl >/dev/null 2>&1; then \
		kubectl logs -n $(NAMESPACE) -l app=agent-ide -f; \
	else \
		docker compose -f agent-compose.yml logs -f; \
	fi

stop-cluster:                ## tear down k3s namespace or compose stack
	@if command -v kubectl >/dev/null 2>&1; then \
		kubectl delete namespace $(NAMESPACE) --ignore-not-found; \
	else \
		docker compose -f agent-compose.yml down; \
	fi

help:
	@grep -E '^[a-z-]+:' Makefile | grep '##' | sed 's/:.*## /\t/'
