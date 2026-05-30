REGISTRY   := ghcr.io
OWNER      := agennext
IMAGE      := $(REGISTRY)/$(OWNER)/agent-ide
TAG        ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo latest)
NAMESPACE  ?= agent-ide
PROFILES   ?=

PROFILE_FLAGS := $(foreach p,$(PROFILES),--profile $(p))

.PHONY: all compose build publish deploy logs down

## all: compose → build → publish → deploy
all: compose build publish deploy

## compose: validate agent-compose.yml
compose:
	@echo "▶  Validating agent-compose.yml..."
	docker compose -f agent-compose.yml $(PROFILE_FLAGS) config --quiet
	@echo "✔  agent-compose.yml is valid"

## build: build the combined image
build:
	@echo "▶  Building $(IMAGE):$(TAG)..."
	docker build -f Containerfile \
		--tag $(IMAGE):$(TAG) \
		--tag $(IMAGE):latest \
		--label "org.opencontainers.image.revision=$(shell git rev-parse HEAD 2>/dev/null)" \
		--label "org.opencontainers.image.created=$(shell date -u +%Y-%m-%dT%H:%M:%SZ)" \
		.
	@echo "✔  Built $(IMAGE):$(TAG)"

## publish: push image to GHCR (docker login ghcr.io first)
publish:
	@echo "▶  Pushing $(IMAGE):$(TAG)..."
	docker push $(IMAGE):$(TAG)
	docker push $(IMAGE):latest
	@echo "✔  Published $(IMAGE):$(TAG)"

## deploy: apply to k3s (or docker compose if k3s not present)
deploy:
	@if command -v k3s >/dev/null 2>&1 || command -v kubectl >/dev/null 2>&1; then \
		echo "▶  Deploying to k3s (namespace: $(NAMESPACE))..."; \
		bash deploy.sh $(TAG) $(foreach p,$(PROFILES),--profile $(p)) --namespace $(NAMESPACE); \
	else \
		echo "▶  k3s not found — deploying via docker compose..."; \
		TAG=$(TAG) docker compose -f agent-compose.yml $(PROFILE_FLAGS) up -d; \
		echo "✔  Running at http://localhost"; \
	fi

## logs: tail container logs
logs:
	@if command -v kubectl >/dev/null 2>&1; then \
		kubectl logs -n $(NAMESPACE) -l app=agent-ide -f; \
	else \
		docker compose -f agent-compose.yml logs -f; \
	fi

## down: stop and remove
down:
	@if command -v kubectl >/dev/null 2>&1; then \
		kubectl delete namespace $(NAMESPACE) --ignore-not-found; \
	else \
		docker compose -f agent-compose.yml down; \
	fi

help:
	@grep -E '^## ' Makefile | sed 's/## /  /'
