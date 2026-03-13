import { getWorkflow, getWork, deleteWorkflow, updateWorkflow } from '../api.js'
import { esc, fmtDate, fmtDuration, badge, setApp, breadcrumb } from '../utils.js'
import { toastError, toastSuccess } from '../components/toast.js'
import { renderDag, updateDagColors } from '../components/dag.js'
import { setCleanup, navigate } from '../router.js'

const TERMINAL = new Set(['succeeded', 'failed', 'cancelled'])
const REFRESH_MS = 3000

export async function renderWorkflow({ ns, id }) {
  setApp(`
    ${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns,href:`#/namespaces/${encodeURIComponent(ns)}`},{label:'Workflows'}])}
    <p aria-busy="true">Loading workflow…</p>
  `)

  let cy = null
  let intervalId = null

  // Fetch both workflow and its Work definition in parallel
  let wf, work
  try {
    wf = await getWorkflow(ns, id)
  } catch (e) {
    toastError(`Workflow not found: ${e.message}`)
    setApp(`${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns,href:`#/namespaces/${encodeURIComponent(ns)}`},{label:'Workflows',href:`#/namespaces/${encodeURIComponent(ns)}/workflows`}])}<div class="empty-state"><p>Workflow not found.</p></div>`)
    return
  }

  try {
    work = await getWork(ns, wf.work_name, wf.work_version)
  } catch (_) {
    work = null // DAG unavailable, degrade gracefully
  }

  renderDetail(ns, wf, work)
  const dagContainer = document.getElementById('dag-container')
  if (dagContainer && work) {
    cy = renderDag(dagContainer, work, wf.steps ?? {}, (stepId) => scrollToStep(stepId))
  }

  // Auto-refresh while running
  if (!TERMINAL.has(wf.status)) {
    intervalId = setInterval(async () => {
      try {
        wf = await getWorkflow(ns, id)
        refreshHeader(wf)
        updateDagColors(cy, wf.steps ?? {})
        refreshSteps(ns, id, wf, work)
        if (TERMINAL.has(wf.status)) {
          clearInterval(intervalId)
          intervalId = null
        }
      } catch (_) {}
    }, REFRESH_MS)

    setCleanup(() => { if (intervalId) clearInterval(intervalId) })
  }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

function renderDetail(ns, wf, work) {
  const nsEnc   = encodeURIComponent(ns)
  const status  = wf.status ?? 'waiting'
  const metrics = wf.metrics ?? {}

  const stepsOrdered = orderedSteps(work, wf.steps ?? {})

  setApp(`
    ${breadcrumb([
      {label:'Namespaces', href:'#/namespaces'},
      {label:ns,           href:`#/namespaces/${nsEnc}`},
      {label:'Workflows',  href:`#/namespaces/${nsEnc}/workflows`},
      {label: wf.id.substring(0,8)+'…'},
    ])}

    <!-- Header card -->
    <article id="wf-header-card">
      ${headerCardInner(wf)}
    </article>

    <!-- Metrics row -->
    <div class="metrics-grid" id="wf-metrics">
      ${metricsInner(metrics, wf)}
    </div>

    <!-- DAG -->
    ${work
      ? `<h4>Execution Graph</h4><div id="dag-container"></div>`
      : `<p class="muted" style="margin-bottom:1.5rem">Work definition not found — DAG unavailable.</p>`
    }

    <!-- Steps -->
    <h4>Steps</h4>
    <div id="steps-list">
      ${stepsInner(stepsOrdered, ns, wf.id)}
    </div>

    <!-- Actions -->
    <div class="row" style="margin-top:1rem">
      ${!TERMINAL.has(status)
        ? `<button class="secondary outline" id="btn-cancel">Cancel Workflow</button>`
        : `<button class="secondary outline" id="btn-delete">Delete Workflow</button>`
      }
      <a href="#/namespaces/${nsEnc}/workflows" class="secondary">← Back to list</a>
    </div>
  `)

  // Cancel / Delete action
  const cancelBtn = document.getElementById('btn-cancel')
  const deleteBtn = document.getElementById('btn-delete')
  if (cancelBtn) {
    cancelBtn.addEventListener('click', async () => {
      if (!confirm('Cancel this workflow?')) return
      try {
        await updateWorkflow(ns, wf.id, { status: 'cancelled' })
        toastSuccess('Workflow cancelled.')
        navigate(`/namespaces/${encodeURIComponent(ns)}/workflows`)
      } catch (e) { toastError(e.message) }
    })
  }
  if (deleteBtn) {
    deleteBtn.addEventListener('click', async () => {
      if (!confirm('Delete this workflow?')) return
      try {
        await deleteWorkflow(ns, wf.id)
        toastSuccess('Workflow deleted.')
        navigate(`/namespaces/${encodeURIComponent(ns)}/workflows`)
      } catch (e) { toastError(e.message) }
    })
  }

  // Step toggle expand/collapse
  attachStepToggleHandlers()
}

function headerCardInner(wf) {
  return `
    <header>
      <div class="row-between">
        <span>
          <strong>${esc(wf.work_name)}</strong>
          <span class="muted"> @ ${esc(wf.work_version)}</span>
        </span>
        ${badge(wf.status ?? 'waiting')}
      </div>
    </header>
    <div class="row" style="font-size:0.88rem;flex-wrap:wrap;gap:1rem">
      <span><span class="muted">ID:</span> <code>${esc(wf.id)}</code></span>
      <span><span class="muted">Created:</span> ${fmtDate(wf.created_at)}</span>
      ${wf.started_at  ? `<span><span class="muted">Started:</span>  ${fmtDate(wf.started_at)}</span>` : ''}
      ${wf.finished_at ? `<span><span class="muted">Finished:</span> ${fmtDate(wf.finished_at)}</span>` : ''}
      ${wf.started_at  ? `<span><span class="muted">Duration:</span> ${fmtDuration(wf.started_at, wf.finished_at)}</span>` : ''}
    </div>
    ${wf.triggers?.cron_id ? `<p class="muted" style="margin-top:0.5rem;margin-bottom:0">Triggered by cron: <code>${esc(wf.triggers.cron_id)}</code></p>` : ''}
  `
}

function metricsInner(metrics, wf) {
  const total     = metrics.steps_total     ?? 0
  const succeeded = metrics.steps_succeeded ?? 0
  const failed    = metrics.steps_failed    ?? 0
  const skipped   = metrics.steps_skipped   ?? 0
  const running   = total - succeeded - failed - skipped
  return `
    <div class="metric-card">
      <div class="metric-value" style="color:var(--status-succeeded)">${succeeded}</div>
      <div class="metric-label">Succeeded</div>
    </div>
    <div class="metric-card">
      <div class="metric-value" style="color:var(--status-running)">${Math.max(0, running)}</div>
      <div class="metric-label">Running</div>
    </div>
    <div class="metric-card">
      <div class="metric-value" style="color:var(--status-failed)">${failed}</div>
      <div class="metric-label">Failed</div>
    </div>
    <div class="metric-card">
      <div class="metric-value" style="color:var(--status-skipped)">${skipped}</div>
      <div class="metric-label">Skipped</div>
    </div>
    <div class="metric-card">
      <div class="metric-value">${total}</div>
      <div class="metric-label">Total</div>
    </div>
  `
}

function stepsInner(stepsOrdered, ns, wfId) {
  if (stepsOrdered.length === 0) {
    return '<p class="muted">No steps yet — workflow has not started.</p>'
  }
  return stepsOrdered.map(({ id, state }) => stepCard(id, state, ns, wfId)).join('')
}

function stepCard(stepId, state, ns, wfId) {
  const status   = state?.status ?? 'pending'
  const attempt  = state?.attempt ?? 1
  const dur      = state?.started_at ? fmtDuration(state.started_at, state.finished_at) : ''
  const logs     = state?.logs ?? []
  const hasLogs  = logs.length > 0
  const hasError = !!state?.error

  const metaParts = []
  if (dur)     metaParts.push(dur)
  if (attempt > 1) metaParts.push(`attempt ${attempt}`)

  const logsHtml = hasLogs
    ? `<pre class="log-viewer">${logs.map(esc).join('\n')}</pre>`
    : '<p class="muted" style="margin:0">No logs captured.</p>'

  const errorHtml = hasError
    ? `<p style="color:var(--status-failed);margin-top:0.5rem">✗ ${esc(state.error)}</p>`
    : ''

  const outputKeys = Object.keys(state?.outputs ?? {})
  const outputHtml = outputKeys.length > 0
    ? `<details style="margin-top:0.5rem"><summary class="muted" style="font-size:0.85rem">Outputs (${outputKeys.length})</summary>
        <pre class="log-viewer" style="max-height:120px">${esc(JSON.stringify(state.outputs, null, 2))}</pre>
      </details>`
    : ''

  return `
    <div class="step-card" id="step-${esc(stepId)}">
      <div class="step-header" data-step="${esc(stepId)}">
        <span class="step-chevron">▶</span>
        ${badge(status)}
        <span class="step-name">${esc(stepId)}</span>
        <span class="step-meta">${metaParts.join(' · ')}</span>
      </div>
      <div class="step-body">
        ${errorHtml}
        ${logsHtml}
        ${outputHtml}
      </div>
    </div>
  `
}

// ── Partial refreshes (avoid full re-render while running) ───────────────────

function refreshHeader(wf) {
  const card = document.getElementById('wf-header-card')
  if (card) card.innerHTML = headerCardInner(wf)
}

function refreshSteps(ns, wfId, wf, work) {
  const container = document.getElementById('steps-list')
  if (!container) return
  const stepsOrdered = orderedSteps(work, wf.steps ?? {})
  // Preserve open state
  const openIds = new Set(
    Array.from(document.querySelectorAll('.step-card.open')).map(el => el.id.replace('step-', ''))
  )
  container.innerHTML = stepsInner(stepsOrdered, ns, wfId)
  openIds.forEach(sid => {
    const card = document.getElementById(`step-${sid}`)
    if (card) card.classList.add('open')
  })
  attachStepToggleHandlers()

  // Refresh metrics
  const metricsEl = document.getElementById('wf-metrics')
  if (metricsEl) metricsEl.innerHTML = metricsInner(wf.metrics ?? {}, wf)
}

function attachStepToggleHandlers() {
  document.querySelectorAll('.step-header').forEach(header => {
    header.addEventListener('click', () => {
      header.closest('.step-card').classList.toggle('open')
    })
  })
}

function scrollToStep(stepId) {
  const el = document.getElementById(`step-${stepId}`)
  if (el) {
    el.classList.add('open')
    el.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
  }
}

// ── Step ordering ─────────────────────────────────────────────────────────────

/** Return steps in topological order if work is available, otherwise alphabetical. */
function orderedSteps(work, stepStates) {
  if (!work?.spec?.steps) {
    return Object.entries(stepStates).map(([id, state]) => ({ id, state }))
  }

  const workSteps = work.spec.steps
  const topoOrder = topoSort(workSteps)

  // Merge with runtime states; include any step in stepStates not in the work definition
  const seen = new Set()
  const result = []
  for (const id of topoOrder) {
    seen.add(id)
    result.push({ id, state: stepStates[id] ?? null })
  }
  for (const id of Object.keys(stepStates)) {
    if (!seen.has(id)) result.push({ id, state: stepStates[id] })
  }
  return result
}

function topoSort(workSteps) {
  const inDegree = {}
  const adj = {}
  for (const s of workSteps) {
    inDegree[s.id] = inDegree[s.id] ?? 0
    adj[s.id] = adj[s.id] ?? []
    for (const dep of s.dependsOn ?? s.depends_on ?? []) {
      adj[dep] = adj[dep] ?? []
      adj[dep].push(s.id)
      inDegree[s.id] = (inDegree[s.id] ?? 0) + 1
    }
  }
  const queue = workSteps.filter(s => inDegree[s.id] === 0).map(s => s.id)
  const result = []
  while (queue.length) {
    const n = queue.shift()
    result.push(n)
    for (const next of adj[n] ?? []) {
      inDegree[next]--
      if (inDegree[next] === 0) queue.push(next)
    }
  }
  return result
}
