REGISTRY   := ghcr.io
OWNER      := agennext
IMAGE      := $(REGISTRY)/$(OWNER)/agent-ide
TAG        ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo latest)
NAMESPACE  ?= agent-ide
PROFILES   ?=

PROFILE_FLAGS := $(foreach p,$(PROFILES),--profile $(p))

.PHONY: all build-code build-container push-image deploy-app

all: build-code build-container push-image deploy-app

build-code:
	yarn --cwd packages/agent-ide-types build
	yarn --cwd packages/agent-ide-backend build
	yarn --cwd extensions/agent-ide-core build
	yarn --cwd applications/browser-app build

build-container:
	@docker compose -f agent-compose.yml $(PROFILE_FLAGS) config --quiet
	docker build -f Containerfile \
		--tag $(IMAGE):$(TAG) \
		--tag $(IMAGE):latest \
		--label "org.opencontainers.image.revision=$(shell git rev-parse HEAD 2>/dev/null)" \
		--label "org.opencontainers.image.created=$(shell date -u +%Y-%m-%dT%H:%M:%SZ)" \
		.

push-image:
	docker push $(IMAGE):$(TAG)
	docker push $(IMAGE):latest

deploy-app:
	@if command -v kubectl >/dev/null 2>&1; then \
		bash deploy.sh $(TAG) $(foreach p,$(PROFILES),--profile $(p)) --namespace $(NAMESPACE); \
	else \
		TAG=$(TAG) docker compose -f agent-compose.yml $(PROFILE_FLAGS) up -d; \
	fi
