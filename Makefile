COMMIT_HASH := $(shell git rev-parse --short HEAD)
TAG := ghcr.io/juspay/decision-engine:sha_$(COMMIT_HASH)

docker-build:
    docker build --platform=linux/amd64 -t $(TAG) .

docker-run:
    docker run --platform=linux/amd64 -v `pwd`/config/docker-configuration.toml:/local/config/development.toml -p 8080:8080 -d $(TAG)

docker-it-run:
    docker run --platform=linux/amd64 -v `pwd`/config/docker-configuration.toml:/local/config/development.toml -it $(TAG) /bin/bash

init:
	docker-compose run --rm db-migrator && docker-compose up open-router

init-pg:
	docker-compose run --rm db-migrator-postgres && docker-compose up open-router-pg
	
run:
	docker-compose up open-router

init-local:
	docker-compose run --rm db-migrator && docker-compose up --build open-router-local

init-local-pg:
	docker-compose run --rm db-migrator-postgres && docker-compose up --build open-router-local

run-local:
	docker-compose up open-router-local

update-config:
	docker-compose run --rm routing-config

stop:
	docker-compose down