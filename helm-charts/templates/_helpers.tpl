{{/*
Expand the name of the chart.
*/}}
{{- define "decision-engine.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "decision-engine.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "decision-engine.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "decision-engine.labels" -}}
helm.sh/chart: {{ include "decision-engine.chart" . }}
{{ include "decision-engine.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "decision-engine.selectorLabels" -}}
app.kubernetes.io/name: {{ include "decision-engine.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "decision-engine.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "decision-engine.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Create the name for the Groovy Runner
*/}}
{{- define "decision-engine.groovyRunnerName" -}}
{{- printf "%s-groovy-runner" (include "decision-engine.fullname" .) }}
{{- end }}

{{/*
Create the name for the PostgreSQL Migration Job
*/}}
{{- define "decision-engine.postgresqlMigrationName" -}}
{{- printf "%s-pg-migration" (include "decision-engine.fullname" .) }}
{{- end }}

{{/*
Create the name for the MySQL Migration Job
*/}}
{{- define "decision-engine.mysqlMigrationName" -}}
{{- printf "%s-mysql-migration" (include "decision-engine.fullname" .) }}
{{- end }}

{{/*
Create the name for the Routing Config Job
*/}}
{{- define "decision-engine.routingConfigName" -}}
{{- printf "%s-routing-config" (include "decision-engine.fullname" .) }}
{{- end }}

{{/*
Define the PostgreSQL hostname
*/}}
{{- define "decision-engine.postgresqlHost" -}}
{{- if .Values.postgresql.enabled }}
{{- printf "%s-postgresql" .Release.Name }}
{{- else }}
{{- .Values.postgresql.hostname | default (printf "%s-postgresql" .Release.Name) }}
{{- end }}
{{- end }}

{{/*
Define the Redis hostname
*/}}
{{- define "decision-engine.redisHost" -}}
{{- if .Values.redis.enabled }}
{{- printf "%s-redis-master" .Release.Name }}
{{- else }}
{{- .Values.redis.hostname | default (printf "%s-redis" .Release.Name) }}
{{- end }}
{{- end }}

{{/*
Generate decision engine config file
*/}}
{{- define "decision-engine.configFile" -}}
[log.console]
enabled = true
level = {{ .Values.decisionEngine.logging.level | quote }}
log_format = {{ .Values.decisionEngine.logging.format | quote }}

[server]
host = {{ .Values.decisionEngine.server.host | quote }}
port = {{ .Values.decisionEngine.server.port }}

[metrics]
host = {{ .Values.decisionEngine.metrics.host | quote }}
port = {{ .Values.decisionEngine.metrics.port }}

[limit]
request_count = {{ .Values.decisionEngine.rateLimit.requestCount }}
duration = {{ .Values.decisionEngine.rateLimit.duration }}

{{- if .Values.decisionEngine.useMySQL }}
[database]
username = {{ .Values.mysql.auth.username | default "root" | quote }}
password = {{ .Values.mysql.auth.password | default "root" | quote }}
host = {{ include "decision-engine.mysqlHost" . | quote }}
port = 3306
dbname = {{ .Values.mysql.auth.database | default "jdb" | quote }}
{{- end }}

{{- if .Values.decisionEngine.usePostgreSQL }}
[pg_database]
pg_username = {{ .Values.postgresql.auth.username | quote }}
pg_password = {{ .Values.postgresql.auth.password | quote }}
pg_host = {{ include "decision-engine.postgresqlHost" . | quote }}
pg_port = 5432
pg_dbname = {{ .Values.postgresql.auth.database | quote }}
{{- end }}

[redis]
host = {{ include "decision-engine.redisHost" . | quote }}
port = 6379
pool_size = 5
reconnect_max_attempts = 5
reconnect_delay = 5
use_legacy_version = false
stream_read_count = 1
auto_pipeline = true
disable_auto_backpressure = false
max_in_flight_commands = 5000
default_command_timeout = 30
unresponsive_timeout = 10
max_feed_count = 200

[cache]
tti = {{ .Values.decisionEngine.cache.tti }}
max_capacity = {{ .Values.decisionEngine.cache.maxCapacity }}

[tenant_secrets]
public = { schema = "public" }

[secrets_management]
secrets_manager = {{ .Values.decisionEngine.secrets.secretsManager | quote }}

[secrets_management.aws_kms]
key_id = {{ .Values.decisionEngine.secrets.awsKms.keyId | quote }}
region = {{ .Values.decisionEngine.secrets.awsKms.region | quote }}

[api_client]
client_idle_timeout = {{ .Values.decisionEngine.apiClient.clientIdleTimeout }}
pool_max_idle_per_host = {{ .Values.decisionEngine.apiClient.poolMaxIdlePerHost }}
identity = {{ .Values.decisionEngine.apiClient.identity | quote }}
{{- end }}

{{/*
Define the MySQL hostname
*/}}
{{- define "decision-engine.mysqlHost" -}}
{{- if .Values.mysql.enabled }}
{{- printf "%s-mysql" .Release.Name }}
{{- else }}
{{- .Values.mysql.hostname | default (printf "%s-mysql" .Release.Name) }}
{{- end }}
{{- end }}
