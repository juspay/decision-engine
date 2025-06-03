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
cd helm-charts/decision-engine
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

The command deploys Decision Engine on the Kubernetes cluster with the default configuration. The [Parameters](#parameters) section lists the parameters that can be configured during installation.

## Uninstalling the Chart

To uninstall/delete the `my-release` deployment:

```bash
helm delete my-release
```

## Parameters

### Common parameters

| Name                | Description                                                                           | Value           |
|---------------------|---------------------------------------------------------------------------------------|-----------------|
| `replicaCount`      | Number of Decision Engine replicas                                                    | `1`             |
| `image.repository`  | Decision Engine image repository                                                      | `decision-engine` |
| `image.tag`         | Decision Engine image tag                                                             | `latest`        |
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
| `decisionEngine.secrets.awsKms.keyId`      | AWS KMS key ID                                             | `"us-west-2"`   |
| `decisionEngine.secrets.awsKms.region`     | AWS KMS region                                             | `"abc"`         |
| `decisionEngine.apiClient.clientIdleTimeout` | API client idle timeout                                  | `90`            |
| `decisionEngine.apiClient.poolMaxIdlePerHost` | API client pool max idle per host                       | `10`            |
| `decisionEngine.apiClient.identity`        | API client identity                                        | `""`            |
| `decisionEngine.routingConfig.enabled`     | Enable routing config                                      | `true`          |

### PostgreSQL Configuration

| Name                              | Description                                                | Value           |
|-----------------------------------|------------------------------------------------------------|-----------------|
| `postgresql.enabled`              | Deploy PostgreSQL                                          | `true`          |
| `postgresql.auth.username`        | PostgreSQL username                                        | `"db_user"`     |
| `postgresql.auth.password`        | PostgreSQL password                                        | `"db_pass"`     |
| `postgresql.auth.database`        | PostgreSQL database name                                   | `"decision_engine_db"` |
| `postgresql.primary.persistence.enabled` | Enable PostgreSQL persistence                       | `true`          |
| `postgresql.primary.persistence.size` | PostgreSQL PVC size                                    | `8Gi`           |

### Redis Configuration

| Name                           | Description                                                | Value           |
|--------------------------------|------------------------------------------------------------|-----------------|
| `redis.enabled`                | Deploy Redis                                               | `true`          |
| `redis.auth.enabled`           | Enable Redis authentication                                | `false`         |
| `redis.master.persistence.enabled` | Enable Redis persistence                               | `true`          |
| `redis.master.persistence.size` | Redis PVC size                                            | `8Gi`           |
| `redis.replica.replicaCount`   | Redis replica count                                        | `1`             |
| `redis.replica.persistence.enabled` | Enable Redis replica persistence                      | `true`          |
| `redis.replica.persistence.size` | Redis replica PVC size                                   | `8Gi`           |

### Groovy Runner Configuration

| Name                                    | Description                                                | Value           |
|-----------------------------------------|------------------------------------------------------------|-----------------|
| `groovyRunner.enabled`                  | Deploy Groovy Runner                                       | `true`          |
| `groovyRunner.image.repository`         | Groovy Runner image repository                             | `"groovy-runner"` |
| `groovyRunner.image.tag`                | Groovy Runner image tag                                    | `"latest"`      |
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

| Name                                      | Description                                                | Value           |
|-------------------------------------------|------------------------------------------------------------|-----------------|
| `dbMigration.enabled`                     | Enable database migration                                  | `true`          |
| `dbMigration.postgresql.enabled`          | Enable PostgreSQL migration                                | `true`          |
| `dbMigration.postgresql.image.repository` | PostgreSQL migration image repository                      | `"postgres"`    |
| `dbMigration.postgresql.image.tag`        | PostgreSQL migration image tag                             | `"latest"`      |
| `dbMigration.postgresql.image.pullPolicy` | PostgreSQL migration image pull policy                     | `"IfNotPresent"` |
| `dbMigration.postgresql.migrations.path`  | PostgreSQL migrations path                                 | `"/app/migrations_pg"` |
| `dbMigration.postgresql.initSqlScript`    | PostgreSQL initial SQL script                              | `"-- This will be the initial migration script"` |
| `dbMigration.mysql.enabled`               | Enable MySQL migration                                     | `false`         |

### Routing Config Configuration

| Name                                     | Description                                                | Value           |
|------------------------------------------|------------------------------------------------------------|-----------------|
| `routingConfig.enabled`                  | Enable routing config job                                  | `true`          |
| `routingConfig.image.repository`         | Routing config image repository                            | `"python"`      |
| `routingConfig.image.tag`                | Routing config image tag                                   | `"3.10-slim"`   |
| `routingConfig.image.pullPolicy`         | Routing config image pull policy                           | `"IfNotPresent"` |
| `routingConfig.command`                  | Routing config command                                     | `["bash", "run_setup.sh"]` |
| `routingConfig.configVolume.enabled`     | Enable routing config volume                               | `true`          |
| `routingConfig.configVolume.mountPath`   | Routing config volume mount path                           | `"/app"`        |

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

## Upgrading

### To 1.0.0

This is the first stable release of the chart.

## Notes

- The chart deploys the Decision Engine with PostgreSQL and Redis by default
- Database migrations are run as Kubernetes Jobs during Helm install/upgrade
- The Groovy Runner is required for the Decision Engine to function properly
