REGISTRY   := ghcr.io
OWNER      := agennext
IMAGE      := $(REGISTRY)/$(OWNER)/agent-ide
TAG        ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo latest)
NAMESPACE  ?= agent-ide
PROFILES   ?=

PROFILE_FLAGS := $(foreach p,$(PROFILES),--profile $(p))

.PHONY: all \
        validate-compose \
        build-image \
        push-image \
        deploy-cluster \
        tail-logs \
        stop-all \
        help

## Full pipeline: validate → build → push → deploy
all: validate-compose build-image push-image deploy-cluster

## validate-compose: lint agent-compose.yml
validate-compose:
	@echo "▶  validate-compose..."
	docker compose -f agent-compose.yml $(PROFILE_FLAGS) config --quiet
	@echo "✔  agent-compose.yml OK"

## build-image: docker build → :SHA + :latest
build-image:
	@echo "▶  build-image $(IMAGE):$(TAG)..."
	docker build -f Containerfile \
		--tag $(IMAGE):$(TAG) \
		--tag $(IMAGE):latest \
		--label "org.opencontainers.image.revision=$(shell git rev-parse HEAD 2>/dev/null)" \
		--label "org.opencontainers.image.created=$(shell date -u +%Y-%m-%dT%H:%M:%SZ)" \
		.
	@echo "✔  built $(IMAGE):$(TAG)"

## push-image: docker push to GHCR  (run: docker login ghcr.io first)
push-image:
	@echo "▶  push-image $(IMAGE):$(TAG)..."
	docker push $(IMAGE):$(TAG)
	docker push $(IMAGE):latest
	@echo "✔  pushed $(IMAGE):$(TAG)"

## deploy-cluster: apply to k3s; falls back to docker compose
deploy-cluster:
	@if command -v k3s >/dev/null 2>&1 || command -v kubectl >/dev/null 2>&1; then \
		echo "▶  deploy-cluster → k3s (namespace: $(NAMESPACE))"; \
		bash deploy.sh $(TAG) $(foreach p,$(PROFILES),--profile $(p)) --namespace $(NAMESPACE); \
	else \
		echo "▶  deploy-cluster → docker compose (k3s not found)"; \
		TAG=$(TAG) docker compose -f agent-compose.yml $(PROFILE_FLAGS) up -d; \
		echo "✔  running at http://localhost"; \
	fi

## tail-logs: stream logs from k3s or docker compose
tail-logs:
	@if command -v kubectl >/dev/null 2>&1; then \
		kubectl logs -n $(NAMESPACE) -l app=agent-ide -f; \
	else \
		docker compose -f agent-compose.yml logs -f; \
	fi

## stop-all: tear down k3s namespace or docker compose stack
stop-all:
	@if command -v kubectl >/dev/null 2>&1; then \
		kubectl delete namespace $(NAMESPACE) --ignore-not-found; \
	else \
		docker compose -f agent-compose.yml down; \
	fi

help:
	@grep -E '^## ' Makefile | sed 's/## /  make /'
