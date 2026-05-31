REGISTRY   := ghcr.io
OWNER      := agennext
IMAGE      := $(REGISTRY)/$(OWNER)/agent-ide
TAG        ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo dev)
NAMESPACE  ?= agent-ide
PROFILES   ?=
PUSH_LATEST ?= false

PROFILE_FLAGS := $(foreach p,$(PROFILES),--profile $(p))

.PHONY: all build push push-latest deploy deploy-compose deploy-k3s

all: build push deploy-compose

build:
	yarn --cwd packages/agent-ide-types build
	yarn --cwd packages/agent-ide-backend build
	yarn --cwd extensions/agent-ide-core build
	yarn --cwd applications/browser-app build
	docker build -f Containerfile \
		--tag $(IMAGE):$(TAG) \
		.

push:
	docker push $(IMAGE):$(TAG)
	@if [ "$(PUSH_LATEST)" = "true" ]; then \
		docker tag $(IMAGE):$(TAG) $(IMAGE):latest; \
		docker push $(IMAGE):latest; \
	fi

push-latest:
	docker tag $(IMAGE):$(TAG) $(IMAGE):latest
	docker push $(IMAGE):latest

deploy:
	@echo "Refusing implicit deploy target. Use one of:"
	@echo "  make deploy-compose TAG=$(TAG)"
	@echo "  make deploy-k3s TAG=$(TAG) NAMESPACE=$(NAMESPACE)"
	@exit 1

deploy-compose:
	TAG=$(TAG) docker compose -f agent-compose.yml $(PROFILE_FLAGS) up -d

deploy-k3s:
	bash deploy.sh $(TAG) $(foreach p,$(PROFILES),--profile $(p)) --namespace $(NAMESPACE)
