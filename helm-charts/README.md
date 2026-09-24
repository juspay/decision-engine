# Decision Engine Helm Chart

A Helm chart for deploying the Decision Engine with PostgreSQL and Redis support on Kubernetes.

## Introduction

This chart bootstraps a [Decision Engine](https://github.com/juspay/decision-engine) deployment on a Kubernetes cluster using the Helm package manager. It optionally deploys a PostgreSQL and Redis instance using the Bitnami charts as dependencies.

## Prerequisites

- Kubernetes 1.19+
- Helm 3.2.0+
- PV provisioner support in the underlying infrastructure (if persistence is needed)

## Installing the Chart

### Option 1: Using the Install Script (Recommended)

The easiest way to install the chart is using the provided install script:

```bash
cd helm-charts
./install.sh
```

By default, this will install the chart with the release name `my-release`. You can specify a different release name or custom values file:

```bash
# Install with a custom release name
./install.sh --name custom-release

# Install with custom values
./install.sh --values values-postgresql.yaml

# Both custom name and values
./install.sh --name custom-release --values values-postgresql.yaml
```

Run `./install.sh --help` for more information.

### Option 2: Manual Installation

If you prefer to install manually, you need to build the dependencies first:

```bash
# Add the bitnami repo for dependencies
helm repo add bitnami https://charts.bitnami.com/bitnami

# Update helm repositories
helm repo update

# Build the dependencies
helm dependency build
```

Then, to install the chart with the release name `my-release`:

```bash
# Install the chart
helm install my-release .
```

To pin a specific on-prem image version (for example `v1.4.35`):

```bash
helm install my-release . \
  --set image.repository=ghcr.io/juspay/decision-engine/postgres \
  --set image.version=v1.4.35 \
  --set image.pullPolicy=Always
```

To use an internal/private registry:

```bash
helm install my-release . \
  --set image.repository=<your-registry>/decision-engine/postgres \
  --set image.version=<your-tag> \
  --set image.pullPolicy=IfNotPresent
```

The command deploys Decision Engine on the Kubernetes cluster with the default configuration. The [Parameters](#parameters) section lists the parameters that can be configured during installation.

## Uninstalling the Chart

To uninstall/delete the `my-release` deployment:

```bash
helm delete my-release
```

## Configuration model

The ConfigMap mounted at `/local/config/development.toml` is the repository's own
`config/development.toml`, symlinked into the chart, so it always matches the source tree it ships
with. It keeps its `localhost` defaults; everything that differs per deployment - database and Redis
hosts, credentials, bind addresses, log level, rate limits, cache, analytics - is overridden with
`DECISION_ENGINE__<section>__<key>` environment variables on the container (the application reads
them with prefix `DECISION_ENGINE` and separator `__`, see `src/config.rs`).

Two consequences worth knowing:

- Reading the ConfigMap alone will show `pg_host = "localhost"`. Check the container's environment
  to see what the pod actually connects to:
  `kubectl get deploy <release>-decision-engine -o jsonpath='{.spec.template.spec.containers[0].env}'`
- Adding a new knob means adding the corresponding env var in `templates/deployment.yaml`; the
  config file is not templated.

After installing, `helm test <release>` runs a pod that calls `/health` through the service. For a
deeper check, `GET /health/diagnostics` with an `x-tenant-id: public` header reports the real
database connection, read, write and delete status.

## Dashboard

The dashboard is the static bundle from `website/`. nginx serves it under `/decision-engine/` and
forwards `/decision-engine/api/` to the Decision Engine service, stripping the prefix on the way.
Both halves matter: the bundle calls the API on its own origin (`website/src/lib/api.ts`) and the
application has no CORS layer, so the two have to answer on one hostname.

```bash
helm upgrade --install my-release ./helm-charts --set dashboard.enabled=true
```

That is all it takes. There is no dashboard image to build or publish: an init container on
`node:20-alpine` fetches the source tarball for `dashboard.build.sourceRef` (defaulting to
`image.version`), runs `npm ci && npm run build`, and writes the result into a volume that an
`nginx:alpine` container serves. Both base images are stock.

What it costs:

- The build needs egress to `github.com` and `registry.npmjs.org`. On a test cluster it took about
  half a minute; with a tighter CPU limit, expect a few minutes.
- It runs again whenever the pod is replaced. Set `dashboard.build.persistence.enabled=true` to keep
  the assets on a volume - the init container then compares `sourceRef` against a marker file it
  wrote and skips the build when they match. With a ReadWriteOnce volume, stay at one replica.
- The dashboard's availability at pod start depends on GitHub and the npm registry being reachable.

With `dashboard.ingress.enabled`, the chart's Ingress gets a single `/decision-engine` rule pointing
at the dashboard service. API calls ride through it, so the Decision Engine needs no public rule of
its own for the dashboard to work.

`/decision-engine/` is baked into the bundle's asset URLs at build time (`website/vite.config.ts`),
so `dashboard.basePath` describes where the assets expect to be served rather than moving them.

| Name                                     | Description                                                | Value           |
|------------------------------------------|------------------------------------------------------------|-----------------|
| `dashboard.enabled`                      | Deploy the dashboard                                       | `false`         |
| `dashboard.replicaCount`                 | Number of dashboard replicas                               | `1`             |
| `dashboard.build.image.repository`       | Image the build runs in                                    | `node`          |
| `dashboard.build.image.tag`              | Build image tag                                            | `20-alpine`     |
| `dashboard.build.refs`                   | Git reference kind: `tags` or `heads`                      | `tags`          |
| `dashboard.build.sourceRef`              | Git reference to build. Defaults to `image.version`        | `""`            |
| `dashboard.build.forceBuild`             | Rebuild even when the volume already holds those assets    | `false`         |
| `dashboard.build.resources`              | Resources for the build container                          | 500m / 1Gi      |
| `dashboard.build.persistence.enabled`    | Keep built assets on a volume, so replaced pods reuse them | `false`         |
| `dashboard.build.persistence.size`       | Size of the assets volume                                  | `1Gi`           |
| `dashboard.nginx.repository`             | Image serving the built assets                             | `nginx`         |
| `dashboard.nginx.tag`                    | nginx image tag                                            | `alpine`        |
| `dashboard.service.port`                 | Dashboard service port                                     | `80`            |
| `dashboard.containerPort`                | Port nginx listens on inside the container                 | `8080`          |
| `dashboard.basePath`                     | Path the bundle is served from. Fixed by the build         | `/decision-engine/` |
| `dashboard.apiBasePath`                  | Path prefix proxied to the Decision Engine                 | `/decision-engine/api` |
| `dashboard.proxyReadTimeout`             | Upstream read timeout for proxied API calls                | `60s`           |
| `dashboard.ingress.enabled`              | Add the `basePath` rule to the chart's Ingress             | `true`          |

## Parameters

### Common parameters

| Name                | Description                                                                           | Value           |
|---------------------|---------------------------------------------------------------------------------------|-----------------|
| `replicaCount`      | Number of Decision Engine replicas                                                    | `1`             |
| `image.repository`  | Decision Engine image repository                                                      | `ghcr.io/juspay/decision-engine/postgres` |
| `image.version`         | Decision Engine image tag. Also the git tag the migration Job pulls migrations from, so keep it on a released `vX.Y.Z` | `v1.4.35`       |
| `image.pullPolicy`  | Decision Engine image pull policy                                                     | `Always`        |
| `imagePullSecrets`  | Image pull secrets                                                                    | `[]`            |
| `nameOverride`      | Override the name of the chart                                                        | `""`            |
| `fullnameOverride`  | Override the full name of the application                                             | `""`            |

### Decision Engine Configuration

| Name                                       | Description                                                | Value           |
|--------------------------------------------|------------------------------------------------------------|-----------------|
| `decisionEngine.useMySQL`                  | Use MySQL instead of PostgreSQL                            | `false`         |
| `decisionEngine.usePostgreSQL`             | Use PostgreSQL                                             | `true`          |
| `decisionEngine.useRedis`                  | Use Redis                                                  | `true`          |
| `decisionEngine.logging.level`             | Logging level                                              | `"DEBUG"`       |
| `decisionEngine.logging.format`            | Logging format                                             | `"default"`     |
| `decisionEngine.server.host`               | Server host                                                | `"0.0.0.0"`     |
| `decisionEngine.server.port`               | Server port                                                | `8080`          |
| `decisionEngine.rateLimit.requestCount`    | Rate limit request count                                   | `1`             |
| `decisionEngine.rateLimit.duration`        | Rate limit duration                                        | `60`            |
| `decisionEngine.cache.tti`                 | Cache TTI in seconds                                       | `7200`          |
| `decisionEngine.cache.maxCapacity`         | Cache maximum capacity                                     | `5000`          |
| `decisionEngine.secrets.openRouterPrivateKey` | Open Router private key                                  | `""`            |
| `decisionEngine.secrets.secretsManager`    | Secrets manager                                            | `"no_encryption"` |
| `decisionEngine.secrets.awsKms.keyId`      | AWS KMS key ID. Only sent when `secretsManager` is not `no_encryption` | `""`            |
| `decisionEngine.secrets.awsKms.region`     | AWS KMS region. Only sent when `secretsManager` is not `no_encryption` | `""`            |
| `decisionEngine.apiClient.clientIdleTimeout` | API client idle timeout                                  | `90`            |
| `decisionEngine.apiClient.poolMaxIdlePerHost` | API client pool max idle per host                       | `10`            |
| `decisionEngine.apiClient.identity`        | API client identity                                        | `""`            |

### PostgreSQL Configuration

| Name                              | Description                                                | Value           |
|-----------------------------------|------------------------------------------------------------|-----------------|
| `postgresql.enabled`              | Deploy the Bitnami PostgreSQL sub-chart                    | `true`          |
| `postgresql.hostname`             | Existing PostgreSQL host, used when the sub-chart is disabled | `""`         |
| `postgresql.image.repository`     | Bitnami's free images now live under `bitnamilegacy`       | `bitnamilegacy/postgresql` |
| `postgresql.image.tag`            | PostgreSQL image tag                                       | `16.1.0-debian-11-r18` |
| `postgresql.auth.username`        | PostgreSQL username                                        | `"db_user"`     |
| `postgresql.auth.password`        | PostgreSQL password                                        | `"db_pass"`     |
| `postgresql.auth.database`        | PostgreSQL database name                                   | `"decision_engine_db"` |
| `postgresql.primary.persistence.enabled` | Enable PostgreSQL persistence                       | `true`          |
| `postgresql.primary.persistence.size` | PostgreSQL PVC size                                    | `8Gi`           |

### Redis Configuration

| Name                           | Description                                                | Value           |
|--------------------------------|------------------------------------------------------------|-----------------|
| `redis.enabled`                | Deploy the Bitnami Redis sub-chart                         | `true`          |
| `redis.hostname`               | Existing Redis host, used when the sub-chart is disabled   | `""`            |
| `redis.image.repository`       | Bitnami's free images now live under `bitnamilegacy`       | `bitnamilegacy/redis` |
| `redis.image.tag`              | Redis image tag                                            | `7.2.3-debian-11-r2` |
| `redis.auth.enabled`           | Enable Redis authentication                                | `false`         |
| `redis.master.persistence.enabled` | Enable Redis persistence                               | `true`          |
| `redis.master.persistence.size` | Redis PVC size                                            | `8Gi`           |
| `redis.replica.replicaCount`   | Redis replica count                                        | `1`             |
| `redis.replica.persistence.enabled` | Enable Redis replica persistence                      | `true`          |
| `redis.replica.persistence.size` | Redis replica PVC size                                   | `8Gi`           |

### Analytics Configuration

The chart can wire Decision Engine to external Kafka and ClickHouse analytics infrastructure. It does not deploy a production Kafka or ClickHouse cluster by default.

| Name | Description | Value |
|------|-------------|-------|
| `analytics.enabled` | Enable analytics config in the Decision Engine deployment | `false` |
| `analytics.kafka.brokers` | Kafka bootstrap brokers used by the API server producer | `"kafka:19092"` |
| `analytics.kafka.apiTopic` | Raw API event topic | `"decision-engine.analytics.api.v1"` |
| `analytics.kafka.domainTopic` | Domain analytics event topic | `"decision-engine.analytics.domain.v1"` |
| `analytics.clickhouse.host` | External ClickHouse host | `"clickhouse"` |
| `analytics.clickhouse.httpPort` | ClickHouse HTTP port used by readers | `8123` |
| `analytics.clickhouse.nativePort` | ClickHouse native port used by init jobs | `9000` |
| `analytics.clickhouse.database` | ClickHouse analytics database | `"decision_engine_analytics"` |
| `analytics.clickhouse.user` | ClickHouse user | `"decision_engine"` |
| `analytics.clickhouse.password` | ClickHouse password | `"decision_engine"` |
| `analytics.clickhouse.bootstrap.enabled` | Run the ClickHouse schema bootstrap job | `false` |

### Groovy Runner Configuration

| Name                                    | Description                                                | Value           |
|-----------------------------------------|------------------------------------------------------------|-----------------|
| `groovyRunner.enabled`                  | Deploy Groovy Runner                                       | `true`          |
| `groovyRunner.image.repository`         | Groovy Runner image repository                             | `"ghcr.io/juspay/decision-engine/groovy-runner"` |
| `groovyRunner.image.version`                | Groovy Runner image tag                                    | `"v1.4.35"`   |
| `groovyRunner.image.pullPolicy`         | Groovy Runner image pull policy                            | `"Always"`      |
| `groovyRunner.service.port`             | Groovy Runner service port                                 | `8085`          |
| `groovyRunner.resources`                | Groovy Runner resources                                    | `{}`            |
| `groovyRunner.healthcheck.enabled`      | Enable Groovy Runner health checks                         | `true`          |
| `groovyRunner.healthcheck.path`         | Groovy Runner health check path                            | `"/health"`     |
| `groovyRunner.healthcheck.initialDelaySeconds` | Groovy Runner health check initial delay            | `10`            |
| `groovyRunner.healthcheck.periodSeconds` | Groovy Runner health check period                         | `5`             |
| `groovyRunner.healthcheck.timeoutSeconds` | Groovy Runner health check timeout                       | `5`             |
| `groovyRunner.healthcheck.failureThreshold` | Groovy Runner health check failure threshold           | `5`             |

### Database Migration Configuration

The migration Job is a post-install/post-upgrade hook. It downloads the release tarball for
`dbMigration.version` (defaulting to `image.version`), installs `diesel_cli` from the upstream
installer and runs the Diesel migrations. Budget several minutes for it, and note that it runs
again on every `helm upgrade` unless you pass `--no-hooks` or set `dbMigration.enabled=false`.

| Name                                      | Description                                                | Value           |
|-------------------------------------------|------------------------------------------------------------|-----------------|
| `dbMigration.enabled`                     | Run the migration Job                                      | `true`          |
| `dbMigration.refs`                        | Git reference kind the migrations come from: `tags` or `heads` | `tags`      |
| `dbMigration.version`                     | Git reference to fetch. Defaults to `image.version`        | `""`            |
| `dbMigration.postgresql.enabled`          | Run the PostgreSQL migration Job                           | `true`          |
| `dbMigration.postgresql.image.registry`   | Registry of the migration base image                       | `docker.io`     |
| `dbMigration.postgresql.image.repository` | Base image for the Job. Needs `bash`, `apt-get` and network access | `debian:trixie-slim` |
| `dbMigration.postgresql.image.pullPolicy` | Migration image pull policy                                | `"IfNotPresent"` |
| `dbMigration.postgresql.migrations.path`  | Migration directory inside the source tree                 | `"migrations_pg"` |
| `dbMigration.postgresql.dieselInstaller`  | Installer used to place `diesel_cli` in the Job            | diesel-rs release installer |
| `dbMigration.mysql.enabled`               | Run the MySQL migration Job                                | `false`         |
| `dbMigration.mysql.image.registry`        | Registry of the MySQL migration base image                 | `docker.io`     |
| `dbMigration.mysql.image.repository`      | Base image for the MySQL migration Job                     | `debian:stable-slim` |
| `dbMigration.mysql.scriptUrls`            | SQL files applied by the MySQL migration Job, in order     | three upstream URLs |

## Examples

### Using with External PostgreSQL

```yaml
# values.yaml
decisionEngine:
  usePostgreSQL: true
  useRedis: true

postgresql:
  enabled: false
  hostname: "external-postgres-host"
  auth:
    username: "external_user"
    password: "external_password"
    database: "external_db"

redis:
  enabled: true
```

### Using with External Redis

```yaml
# values.yaml
decisionEngine:
  usePostgreSQL: true
  useRedis: true

postgresql:
  enabled: true

redis:
  enabled: false
  hostname: "external-redis-host"
```

### Using External Kafka And ClickHouse Analytics

```yaml
analytics:
  enabled: true
  kafka:
    brokers: "kafka-bootstrap.analytics.svc.cluster.local:9092"
    apiTopic: "decision-engine.analytics.api.v1"
    domainTopic: "decision-engine.analytics.domain.v1"
  clickhouse:
    host: "clickhouse.analytics.svc.cluster.local"
    httpPort: 8123
    nativePort: 9000
    database: "decision_engine_analytics"
    user: "decision_engine"
    password: "<from-secret>"
    bootstrap:
      enabled: true
```

## Upgrading

### To 1.0.0

This is the first stable release of the chart.

## Notes

- The chart deploys the Decision Engine with PostgreSQL and Redis by default
- Database migrations are run as Kubernetes Jobs during Helm install/upgrade
- The Groovy Runner is required for the Decision Engine to function properly
