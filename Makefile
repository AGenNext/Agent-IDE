REGISTRY   := ghcr.io
OWNER      := agennext
IMAGE      := $(REGISTRY)/$(OWNER)/agent-ide
TAG        ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo latest)
NAMESPACE  ?= agent-ide
PROFILES   ?=

PROFILE_FLAGS := $(foreach p,$(PROFILES),--profile $(p))

.PHONY: all build push deploy

all: build push deploy

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
