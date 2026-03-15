# Orkester Helm Chart

Deploys [Orkester](../../README.md), a workflow orchestration engine, onto Kubernetes.

## Prerequisites

- Kubernetes 1.27+
- Helm 3.10+

## Install

```bash
# From local checkout
helm install orkester ./helm-charts/orkester \
  --namespace orkester \
  --create-namespace
```

## Uninstall

```bash
helm uninstall orkester --namespace orkester
```

The PersistentVolumeClaim is **not** deleted automatically. Remove it manually if you no longer need the data:

```bash
kubectl delete pvc orkester --namespace orkester
```

---

## Key Configuration

### Image

```yaml
image:
  repository: ghcr.io/your-org/orkester
  tag: "1.0.0"
  pullPolicy: IfNotPresent
```

### Persistence

Orkester uses a file-based workflow persistence backend by default. The PVC is enabled and sized at 1 Gi.

```yaml
persistence:
  enabled: true
  storageClass: ""   # use cluster default
  size: 5Gi
  mountPath: /orkester/data
```

Set `persistence.enabled: false` only when you configure an external persistence backend via `config.servers.workflows.persistence`.

### RBAC for the Kubernetes Executor

The kubernetes executor (`plugin-k8s`) creates ephemeral Pods to run tasks. It needs Pod CRUD permissions. By default, cluster-wide access is granted:

```yaml
rbac:
  create: true
  clusterScoped: true
```

To restrict to specific namespaces only, set:

```yaml
rbac:
  clusterScoped: false
  allowedNamespaces:
    - jobs
    - etl
```

### Ingress

```yaml
ingress:
  enabled: true
  className: nginx
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
  hosts:
    - host: orkester.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: orkester-tls
      hosts: [orkester.example.com]
```

### Network Policy

Disabled by default. Enable to restrict inbound traffic to the configured port only:

```yaml
networkPolicy:
  enabled: true
  # Allow scraping from Prometheus pods in the monitoring namespace
  additionalIngress:
    - from:
        - namespaceSelector:
            matchLabels:
              kubernetes.io/metadata.name: monitoring
      ports:
        - port: 8080
          protocol: TCP
```

### Pod Disruption Budget

```yaml
podDisruptionBudget:
  enabled: true
  minAvailable: 1
```

### Overriding Orkester Config

The entire `config:` block is rendered verbatim into a ConfigMap and mounted at `/orkester/config.yaml`. Override any sub-key:

```yaml
config:
  servers:
    workflows:
      scheduler_interval_seconds: 10
      persistence:
        type: file-persistence
        root-dir: /orkester/data/workflows
  logging:
    console:
      filters:
        - level: DEBUG
```

---

## Values Reference

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `replicaCount` | int | `1` | Pod replicas. Use 1 with file persistence. |
| `image.repository` | string | `orkester` | Image repository. |
| `image.tag` | string | `latest` | Image tag. |
| `image.pullPolicy` | string | `IfNotPresent` | Image pull policy. |
| `service.type` | string | `ClusterIP` | Kubernetes Service type. |
| `service.port` | int | `80` | Service port. |
| `service.containerPort` | int | `8080` | Container port (must match `config.servers.rest_api.port`). |
| `persistence.enabled` | bool | `true` | Enable PVC for workflow state. |
| `persistence.size` | string | `1Gi` | PVC size. |
| `rbac.create` | bool | `true` | Create RBAC resources. |
| `rbac.clusterScoped` | bool | `true` | Grant cluster-wide Pod access (needed for kubernetes executor). |
| `networkPolicy.enabled` | bool | `false` | Enable NetworkPolicy. |
| `podDisruptionBudget.enabled` | bool | `false` | Enable PodDisruptionBudget. |
| `ingress.enabled` | bool | `false` | Enable Ingress. |
| `config` | object | see Values.yaml | Full orkester `config.yaml` content. |
