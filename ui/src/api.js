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
export const getHealth  = () => req('/health')
export const getServers = () => req('/servers')
export const getPlugins = () => req('/plugins')

// ── Workspace ─────────────────────────────────────────────────────────────────
export const listNamespaces = ()             => req('/namespaces')
export const listTasks      = (ns)           => req(`/namespaces/${enc(ns)}/tasks`)
export const getTask        = (ns, n, v)     => req(`/namespaces/${enc(ns)}/tasks/${enc(n)}/${enc(v)}`)
export const listWorks      = (ns)           => req(`/namespaces/${enc(ns)}/works`)
export const getWork        = (ns, n, v)     => req(`/namespaces/${enc(ns)}/works/${enc(n)}/${enc(v)}`)

// ── Workflows ─────────────────────────────────────────────────────────────────
export const listWorkflows  = (ns)           => req(`/namespaces/${enc(ns)}/workflows`)
export const getWorkflow    = (ns, id)       => req(`/namespaces/${enc(ns)}/workflows/${enc(id)}`)
export const createWorkflow = (ns, body)     => req(`/namespaces/${enc(ns)}/workflows`,       { method: 'POST', ...json(body) })
export const updateWorkflow = (ns, id, body) => req(`/namespaces/${enc(ns)}/workflows/${enc(id)}`, { method: 'PUT',  ...json(body) })
export const deleteWorkflow = (ns, id)       => req(`/namespaces/${enc(ns)}/workflows/${enc(id)}`, { method: 'DELETE' })

export const getWorkflowSteps = (ns, id)              => req(`/namespaces/${enc(ns)}/workflows/${enc(id)}/steps`)
export const getStepLogs      = (ns, wfId, stepId)    => req(`/namespaces/${enc(ns)}/workflows/${enc(wfId)}/steps/${enc(stepId)}/logs`)

// ── Crons ─────────────────────────────────────────────────────────────────────
export const listCrons   = (ns)           => req(`/namespaces/${enc(ns)}/crons`)
export const getCron     = (ns, id)       => req(`/namespaces/${enc(ns)}/crons/${enc(id)}`)
export const createCron  = (ns, body)     => req(`/namespaces/${enc(ns)}/crons`,           { method: 'POST', ...json(body) })
export const updateCron  = (ns, id, body) => req(`/namespaces/${enc(ns)}/crons/${enc(id)}`, { method: 'PUT',  ...json(body) })
export const deleteCron  = (ns, id)       => req(`/namespaces/${enc(ns)}/crons/${enc(id)}`, { method: 'DELETE' })

const enc = encodeURIComponent
