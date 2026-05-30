REGISTRY   := ghcr.io
OWNER      := agennext
IMAGE      := $(REGISTRY)/$(OWNER)/agent-ide
TAG        ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo latest)
NAMESPACE  ?= agent-ide
PROFILES   ?=

PROFILE_FLAGS := $(foreach p,$(PROFILES),--profile $(p))

.PHONY: all compose build push deploy logs down help

all: compose build push deploy   ## compose → build → push → deploy

compose:                         ## validate agent-compose.yml
	@echo "▶  compose..."
	docker compose -f agent-compose.yml $(PROFILE_FLAGS) config --quiet
	@echo "✔  OK"

build:                           ## docker build → :SHA + :latest
	@echo "▶  build $(IMAGE):$(TAG)..."
	docker build -f Containerfile \
		--tag $(IMAGE):$(TAG) \
		--tag $(IMAGE):latest \
		--label "org.opencontainers.image.revision=$(shell git rev-parse HEAD 2>/dev/null)" \
		--label "org.opencontainers.image.created=$(shell date -u +%Y-%m-%dT%H:%M:%SZ)" \
		.
	@echo "✔  $(IMAGE):$(TAG)"

push:                            ## docker push to GHCR
	@echo "▶  push $(IMAGE):$(TAG)..."
	docker push $(IMAGE):$(TAG)
	docker push $(IMAGE):latest
	@echo "✔  pushed"

deploy:                          ## deploy to k3s (falls back to docker compose)
	@if command -v k3s >/dev/null 2>&1 || command -v kubectl >/dev/null 2>&1; then \
		echo "▶  deploy → k3s ($(NAMESPACE))"; \
		bash deploy.sh $(TAG) $(foreach p,$(PROFILES),--profile $(p)) --namespace $(NAMESPACE); \
	else \
		echo "▶  deploy → docker compose"; \
		TAG=$(TAG) docker compose -f agent-compose.yml $(PROFILE_FLAGS) up -d; \
		echo "✔  http://localhost"; \
	fi

logs:                            ## tail logs
	@if command -v kubectl >/dev/null 2>&1; then \
		kubectl logs -n $(NAMESPACE) -l app=agent-ide -f; \
	else \
		docker compose -f agent-compose.yml logs -f; \
	fi

down:                            ## stop and remove
	@if command -v kubectl >/dev/null 2>&1; then \
		kubectl delete namespace $(NAMESPACE) --ignore-not-found; \
	else \
		docker compose -f agent-compose.yml down; \
	fi

help:
	@grep -E '##' Makefile | grep -v grep | sed 's/:.*## /\t/'
