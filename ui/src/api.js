// ── HTTP client ────────────────────────────────────────────────────────────
//
// All requests go through req(). Throws an Error with a user-readable message
// on non-2xx responses. Callers should catch and display via toastError().

const BASE = `${window.ORKESTER_API_BASE ?? ''}/v1`
const enc  = encodeURIComponent

async function req(path, init) {
  const res  = await fetch(BASE + path, init)
  if (res.status === 204) return null
  const data = await res.json()
  if (!res.ok) throw new Error(data?.error ?? data?.message ?? `HTTP ${res.status}`)
  return data
}

const json = (body) => ({
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(body),
})

// ── System / Health ────────────────────────────────────────────────────────
// GET /v1/health → { status, version, uptime_secs }
export const getHealth         = ()     => req('/health')
// GET /v1/host/plugins → { plugins[] }
export const getHostPlugins    = ()     => req('/host/plugins')
// GET /v1/host/components → { components[] }
export const getHostComponents = ()     => req('/host/components')
// GET /v1/host/registry/components → [ { name, kind } ]
export const getHostRegistry   = ()     => req('/host/registry/components')

// ── Metrics ────────────────────────────────────────────────────────────────
// GET /v1/metrics/snapshot → { metrics: { name: value } }
export const getMetricsSnapshot = ()   => req('/metrics/snapshot')
// GET /v1/metrics/history  → { metrics: { name: [{timestamp_ms, value}] } }
export const getMetricsHistory  = ()   => req('/metrics/history')

// ── Catalog: built-in convenience endpoints ────────────────────────────────
// GET /v1/namespaces → { namespaces[] }
export const listNamespaces    = ()             => req('/namespaces')
// GET /v1/namespaces/:ns/works → { works[] }
export const listWorks         = (ns)           => req(`/namespaces/${enc(ns)}/works`)
// GET /v1/namespaces/:ns/works/:name/:version → work doc
export const getWork           = (ns, n, v)     => req(`/namespaces/${enc(ns)}/works/${enc(n)}/${enc(v)}`)
// GET /v1/namespaces/:ns/tasks → { tasks[] }
export const listTasks         = (ns)           => req(`/namespaces/${enc(ns)}/tasks`)
// GET /v1/namespaces/:ns/tasks/:name/:version → task doc
export const getTask           = (ns, n, v)     => req(`/namespaces/${enc(ns)}/tasks/${enc(n)}/${enc(v)}`)

// ── Catalog: generic resource API ─────────────────────────────────────────
// These endpoints expose the full CatalogServer document store.
// Query params: kind, namespace, name (all optional, used as filters).
// GET /v1/catalog/resources?kind=X&namespace=X&name=X → { resources[] }
export const searchResources  = (params = {}) => {
  const qs = new URLSearchParams(Object.fromEntries(
    Object.entries(params).filter(([, v]) => v != null && v !== '')
  )).toString()
  return req(`/catalog/resources${qs ? '?' + qs : ''}`)
}
// GET /v1/catalog/resources/:id → resource doc
export const getResource      = (id)          => req(`/catalog/resources/${enc(id)}`)
// POST /v1/catalog/resources → resource doc
export const createResource   = (body)        => req('/catalog/resources',          { method: 'POST',   ...json(body) })
// PUT /v1/catalog/resources/:id → resource doc
export const updateResource   = (id, body)   => req(`/catalog/resources/${enc(id)}`, { method: 'PUT',    ...json(body) })
// DELETE /v1/catalog/resources/:id → null
export const deleteResource   = (id)          => req(`/catalog/resources/${enc(id)}`, { method: 'DELETE' })

// ── Workflow: WorkRuns ─────────────────────────────────────────────────────
// POST /v1/workflow/trigger { workRef, inputs, workRunnerRef? } → { status }
export const triggerWork      = (body)        => req('/workflow/trigger',               { method: 'POST', ...json(body) })
// GET /v1/workflow/work-runs?namespace=X → { work_runs[] }
export const listWorkRuns     = (ns)          => req(`/workflow/work-runs${ns ? '?namespace=' + enc(ns) : ''}`)
// GET /v1/workflow/work-runs/:name → WorkRun doc
export const getWorkRun       = (name)        => req(`/workflow/work-runs/${enc(name)}`)
// POST /v1/workflow/work-runs/:name/cancel → null
export const cancelWorkRun    = (name)        => req(`/workflow/work-runs/${enc(name)}/cancel`, { method: 'POST' })

// ── Workflow: TaskRuns ─────────────────────────────────────────────────────
// GET /v1/workflow/task-runs?namespace=X → { task_runs[] }
export const listTaskRuns     = (ns)          => req(`/workflow/task-runs${ns ? '?namespace=' + enc(ns) : ''}`)
// GET /v1/workflow/task-runs/:name → TaskRun doc
export const getTaskRun       = (name)        => req(`/workflow/task-runs/${enc(name)}`)

// ── Workflow: WorkRunners ──────────────────────────────────────────────────
// GET /v1/workflow/work-runners?namespace=X → { work_runners[] }
export const listWorkRunners  = (ns)          => req(`/workflow/work-runners${ns ? '?namespace=' + enc(ns) : ''}`)
// GET /v1/workflow/work-runners/:name → WorkRunner doc
export const getWorkRunner    = (name)        => req(`/workflow/work-runners/${enc(name)}`)
// POST /v1/workflow/work-runners → WorkRunner doc
export const createWorkRunner = (body)        => req('/workflow/work-runners',           { method: 'POST',   ...json(body) })
// DELETE /v1/workflow/work-runners/:name → null
export const deleteWorkRunner = (name)        => req(`/workflow/work-runners/${enc(name)}`, { method: 'DELETE' })

// ── Workflow: TaskRunners ──────────────────────────────────────────────────
// GET /v1/workflow/task-runners?namespace=X → { task_runners[] }
export const listTaskRunners  = (ns)          => req(`/workflow/task-runners${ns ? '?namespace=' + enc(ns) : ''}`)
// GET /v1/workflow/task-runners/:name → TaskRunner doc
export const getTaskRunner    = (name)        => req(`/workflow/task-runners/${enc(name)}`)

// ── Workflow: Crons ────────────────────────────────────────────────────────
// GET /v1/workflow/crons?namespace=X → { crons[] }
export const listCrons        = (ns)          => req(`/workflow/crons${ns ? '?namespace=' + enc(ns) : ''}`)
// POST /v1/workflow/crons → null  (create or update by name)
export const registerCron     = (body)        => req('/workflow/crons',                  { method: 'POST',   ...json(body) })
// DELETE /v1/workflow/crons/:name → null
export const unregisterCron   = (name)        => req(`/workflow/crons/${enc(name)}`,      { method: 'DELETE' })

// ── Artifacts ─────────────────────────────────────────────────────────────
// Fetch raw content from an artifact URI (e.g. logsRef.stdout / .stderr).
// The URI may be an absolute URL or a relative /v1/... path.
export async function fetchArtifact(uri) {
  if (!uri) return null
  const url = uri.startsWith('http') ? uri : BASE + uri.replace(/^\/v1/, '')
  const res = await fetch(url)
  if (!res.ok) throw new Error(`Artifact fetch failed: HTTP ${res.status}`)
  return res.text()
}
