COMMIT_HASH := $(shell git rev-parse --short HEAD)
TAG := ghcr.io/juspay/decision-engine:sha_$(COMMIT_HASH)

DECISION_ENGINE_TAG ?= v1.4
GROOVY_RUNNER_TAG ?= v1.4

docker-build:
	docker build --platform=linux/amd64 -t $(TAG) .

docker-run:
	docker run --platform=linux/amd64 -v `pwd`/config/docker-configuration.toml:/local/config/development.toml -p 8080:8080 -d $(TAG)

docker-it-run:
	docker run --platform=linux/amd64 -v `pwd`/config/docker-configuration.toml:/local/config/development.toml -it $(TAG) /bin/bash

init-mysql-ghcr:
	DECISION_ENGINE_TAG=$(DECISION_ENGINE_TAG) GROOVY_RUNNER_TAG=$(GROOVY_RUNNER_TAG) docker compose --profile mysql-ghcr up -d

init-pg-ghcr:
	DECISION_ENGINE_TAG=$(DECISION_ENGINE_TAG) GROOVY_RUNNER_TAG=$(GROOVY_RUNNER_TAG) docker compose --profile postgres-ghcr up -d

init-mysql-local:
	docker compose --profile mysql-local up -d --build

init-pg-local:
	docker compose --profile postgres-local up -d --build

run-mysql-ghcr:
	DECISION_ENGINE_TAG=$(DECISION_ENGINE_TAG) GROOVY_RUNNER_TAG=$(GROOVY_RUNNER_TAG) docker compose --profile mysql-ghcr up -d open-router-mysql-ghcr

run-pg-ghcr:
	DECISION_ENGINE_TAG=$(DECISION_ENGINE_TAG) GROOVY_RUNNER_TAG=$(GROOVY_RUNNER_TAG) docker compose --profile postgres-ghcr up -d open-router-pg-ghcr

run-mysql-local:
	docker compose --profile mysql-local up -d --build open-router-mysql-local

run-pg-local:
	docker compose --profile postgres-local up -d --build open-router-pg-local

init-pg-monitor:
	DECISION_ENGINE_TAG=$(DECISION_ENGINE_TAG) GROOVY_RUNNER_TAG=$(GROOVY_RUNNER_TAG) docker compose --profile postgres-ghcr --profile monitoring up -d

init-local-pg-monitor:
	docker compose --profile postgres-local --profile monitoring up -d --build

update-config:
	docker compose --profile mysql-ghcr run --rm routing-config

stop:
	docker compose down

# Backward-compatible aliases
init: init-mysql-ghcr
init-pg: init-pg-ghcr
run: run-mysql-ghcr
init-local: init-mysql-local
init-local-pg: init-pg-local
run-local: run-mysql-local
