docker-build:
	docker build -t open_router .

docker-run:
	docker run -v `pwd`/config/docker-configuration.toml:/local/config/development.toml -p 8080:8080 -d open_router

docker-it-run:
	docker run -v `pwd`/config/docker-configuration.toml:/local/config/development.toml -it open_router /bin/bash
