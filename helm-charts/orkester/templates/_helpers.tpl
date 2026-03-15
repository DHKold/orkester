{{/*
Build the fully-qualified image reference, preferring digest over tag.
Falls back to Chart.AppVersion when image.tag is empty.
*/}}
{{- define "orkester.image" -}}
{{- $registry   := .Values.image.registry -}}
{{- $repository := .Values.image.repository -}}
{{- $tag        := .Values.image.tag | default .Chart.AppVersion -}}
{{- $digest     := .Values.image.digest -}}
{{- if $digest -}}
  {{- if $registry -}}{{- printf "%s/%s@%s" $registry $repository $digest -}}
  {{- else -}}{{- printf "%s@%s" $repository $digest -}}{{- end -}}
{{- else -}}
  {{- if $registry -}}{{- printf "%s/%s:%s" $registry $repository $tag -}}
  {{- else -}}{{- printf "%s:%s" $repository $tag -}}{{- end -}}
{{- end -}}
{{- end }}

{{/*
Expand the name of the chart.
*/}}
{{- define "orkester.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "orkester.fullname" -}}
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
Create chart label value.
*/}}
{{- define "orkester.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels.
*/}}
{{- define "orkester.labels" -}}
helm.sh/chart: {{ include "orkester.chart" . }}
{{ include "orkester.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- with .Values.commonLabels }}
{{ toYaml . | trim }}
{{- end }}
{{- end }}

{{/*
Selector labels.
*/}}
{{- define "orkester.selectorLabels" -}}
app.kubernetes.io/name: {{ include "orkester.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
ServiceAccount name.
*/}}
{{- define "orkester.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "orkester.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}
