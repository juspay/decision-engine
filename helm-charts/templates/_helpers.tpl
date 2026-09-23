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
Create the name for the analytics ClickHouse bootstrap configmap
*/}}
{{- define "decision-engine.analyticsClickhouseConfigName" -}}
{{- printf "%s-analytics-clickhouse" (include "decision-engine.fullname" .) }}
{{- end }}

{{/*
Create the name for the analytics ClickHouse bootstrap job
*/}}
{{- define "decision-engine.analyticsClickhouseJobName" -}}
{{- printf "%s-analytics-clickhouse-init" (include "decision-engine.fullname" .) }}
{{- end }}

{{/*
Define the PostgreSQL hostname
*/}}
{{- define "decision-engine.postgresqlHost" -}}
{{- if .Values.postgresql.enabled }}
{{- if .Values.postgresql.fullnameOverride }}
{{- .Values.postgresql.fullnameOverride }}
{{- else }}
{{- printf "%s-postgresql" .Release.Name }}
{{- end }}
{{- else }}
{{- .Values.postgresql.hostname | default (printf "%s-postgresql" .Release.Name) }}
{{- end }}
{{- end }}

{{/*
Define the Redis hostname
*/}}
{{- define "decision-engine.redisHost" -}}
{{- if .Values.redis.enabled }}
{{- if .Values.redis.fullnameOverride }}
{{- printf "%s-master" .Values.redis.fullnameOverride }}
{{- else }}
{{- printf "%s-redis-master" .Release.Name }}
{{- end }}
{{- else }}
{{- .Values.redis.hostname | default (printf "%s-redis" .Release.Name) }}
{{- end }}
{{- end }}

{{/*
Define the MySQL hostname
*/}}
{{- define "decision-engine.mysqlHost" -}}
{{- if .Values.mysql.enabled }}
{{- if .Values.mysql.fullnameOverride }}
{{- .Values.mysql.fullnameOverride }}
{{- else }}
{{- printf "%s-mysql" .Release.Name }}
{{- end }}
{{- else }}
{{- .Values.mysql.hostname | default (printf "%s-mysql" .Release.Name) }}
{{- end }}
{{- end }}
