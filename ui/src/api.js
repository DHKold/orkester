const BASE = `${window.ORKESTER_API_BASE ?? ''}/v1`

async function req(path, init) {
  const res = await fetch(BASE + path, init)
  if (res.status === 204) return null
  const data = await res.json()
  if (!res.ok) throw new Error(data?.error ?? `HTTP ${res.status}`)
  return data
}

const json = (body) => ({
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(body),
})

// ── Management ────────────────────────────────────────────────────────────────
export const getHealth         = () => req('/health')
export const getHostPlugins    = () => req('/host/plugins')
export const getHostComponents = () => req('/host/components')
export const getHostRegistry   = () => req('/host/registry/components')

// ── Workspace ─────────────────────────────────────────────────────────────────
export const listNamespaces = ()             => req('/namespaces')
export const listTasks      = (ns)           => req(`/namespaces/${enc(ns)}/tasks`)
export const getTask        = (ns, n, v)     => req(`/namespaces/${enc(ns)}/tasks/${enc(n)}/${enc(v)}`)
export const listWorks      = (ns)           => req(`/namespaces/${enc(ns)}/works`)
export const getWork        = (ns, n, v)     => req(`/namespaces/${enc(ns)}/works/${enc(n)}/${enc(v)}`)

// ── WorkRuns (Workflow Server) ────────────────────────────────────────────────
export const triggerWork      = (body)       => req('/workflow/trigger',      { method: 'POST', ...json(body) })
export const listWorkRuns     = ()           => req('/workflow/work-runs')
export const getWorkRun       = (name)       => req(`/workflow/work-runs/${enc(name)}`)
export const cancelWorkRun    = (name)       => req(`/workflow/work-runs/${enc(name)}/cancel`, { method: 'POST' })
export const listTaskRuns     = ()           => req('/workflow/task-runs')
export const getTaskRun       = (name)       => req(`/workflow/task-runs/${enc(name)}`)

// ── Crons ─────────────────────────────────────────────────────────────────────
export const listCrons        = ()           => req('/workflow/crons')
export const registerCron     = (body)       => req('/workflow/crons',         { method: 'POST', ...json(body) })
export const unregisterCron   = (name)       => req(`/workflow/crons/${enc(name)}`, { method: 'DELETE' })
// Aliases used by crons.js
export const createCron       = (ns, body)   => registerCron(body)
export const updateCron       = (ns, body)   => registerCron(body)
export const deleteCron       = (ns, name)   => unregisterCron(name)

// ── Loader ────────────────────────────────────────────────────────────────────
export const getLoaderMetrics = ()           => req('/loader/metrics')

const enc = encodeURIComponent
