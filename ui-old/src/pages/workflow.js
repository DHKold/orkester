import { getWorkRun, cancelWorkRun, listWorks } from '../api.js'
import { esc, fmtDate, fmtDuration, badge, setApp, breadcrumb } from '../utils.js'
import { toastError, toastSuccess } from '../components/toast.js'
import { renderDag, updateDagColors } from '../components/dag.js'
import { setCleanup, navigate } from '../router.js'
import {
  stepsHtml, runLogsHtml,
  attachStepToggleHandlers, attachFilterHandlers, scrollToStep, restoreOpenSteps,
} from '../components/workflow-steps.js'

const TERMINAL = new Set(['succeeded', 'failed', 'cancelled'])
const REFRESH_MS = 3000

export async function renderWorkflow({ ns, id }) {
  setApp(`${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns,href:`#/namespaces/${encodeURIComponent(ns)}`},{label:'Workflows'}])}<p aria-busy="true">Loading workflow…</p>`)
  let wr
  try {
    wr = await getWorkRun(id)
  } catch (e) {
    toastError(`Workflow not found: ${e.message}`)
    setApp(`${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns,href:`#/namespaces/${encodeURIComponent(ns)}`},{label:'Workflows',href:`#/namespaces/${encodeURIComponent(ns)}/workflows`}])}<div class="empty-state"><p>Workflow run not found.</p></div>`)
    return
  }
  const workDef = await loadWorkDef(wr, ns)
  renderDetail(ns, wr, workDef)
  let cy = null
  const dagContainer = document.getElementById('dag-container')
  if (dagContainer) cy = renderDag(dagContainer, workDef ?? buildWorkFromRun(wr), buildStepStateMap(wr), scrollToStep)
  document.getElementById('btn-dag-reset')?.addEventListener('click', () => cy?.fit())
  if (!TERMINAL.has(wr.status?.state ?? 'pending')) startRefreshLoop(id, cy)
}

async function loadWorkDef(wr, ns) {
  try {
    const workRef = wr.spec?.workRef ?? ''
    const [wns, wname] = workRef.includes('/') ? workRef.split('/') : [ns, workRef]
    const { works } = await listWorks(wns)
    return (works ?? []).find(w => w.name === wname) ?? null
  } catch (_) { return null }
}

function startRefreshLoop(id, cy) {
  let timerId = setInterval(async () => {
    try {
      const wr = await getWorkRun(id)
      refreshHeader(wr)
      updateDagColors(cy, buildStepStateMap(wr))
      refreshSteps(wr)
      refreshRunLogs(wr)
      if (TERMINAL.has(wr.status?.state ?? '')) { clearInterval(timerId); timerId = null }
    } catch (_) {}
  }, REFRESH_MS)
  setCleanup(() => { if (timerId) clearInterval(timerId) })
}

// ── Data adapters ─────────────────────────────────────────────────────────────

function buildStepStateMap(wr) {
  const map = {}
  for (const s of wr.status?.steps ?? []) map[s.name] = { status: s.state, attempts: s.attempts }
  return map
}

function buildWorkFromRun(wr) {
  const steps = (wr.status?.steps ?? []).map(s => ({ name: s.name, depends_on: [] }))
  return { spec: { steps } }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

function renderDetail(ns, wr, workDef) {
  const nsEnc  = encodeURIComponent(ns)
  const status = wr.status?.state ?? 'pending'
  const steps  = wr.status?.steps ?? []

  setApp(`
    ${breadcrumb([
      {label:'Namespaces', href:'#/namespaces'},
      {label:ns,           href:`#/namespaces/${nsEnc}`},
      {label:'Workflows',  href:`#/namespaces/${nsEnc}/workflows`},
      {label: wr.name.substring(0,8)+'…'},
    ])}
    <div class="row-between" style="margin-bottom:1rem"><div></div>
      <div class="row" style="gap:0.5rem">
        ${!TERMINAL.has(status) ? `<button class="outline btn-xs" id="btn-cancel">Cancel</button>` : ''}
      </div>
    </div>
    <article id="wf-header-card">${headerCardInner(wr)}</article>
    <div class="metrics-grid" id="wf-metrics">${metricsInner(wr)}</div>
    <div class="row-between" style="align-items:center;margin-bottom:0.25rem">
      <h4 style="margin:0">Execution Graph</h4>
      <button id="btn-dag-reset" class="outline btn-xs">↺ Reset view</button>
    </div>
    <div id="dag-container"></div>
    <h4>Steps</h4>
    <div id="steps-list">${stepsHtml(steps)}</div>
    <details style="margin-top:1.5rem">
      <summary><strong>Run Logs</strong></summary>
      <div id="run-logs" style="margin-top:0.5rem">${runLogsHtml(wr.status?.logs ?? [])}</div>
    </details>
  `)

  const cancelBtn = document.getElementById('btn-cancel')
  if (cancelBtn) {
    cancelBtn.addEventListener('click', async () => {
      if (!confirm('Cancel this workflow run?')) return
      try {
        await cancelWorkRun(wr.name)
        toastSuccess('Workflow cancelled.')
        navigate(`/namespaces/${encodeURIComponent(ns)}/workflows`)
      } catch (e) { toastError(e.message) }
    })
  }
  attachStepToggleHandlers()
  attachFilterHandlers(steps)
}

function headerCardInner(wr) {
  const st  = wr.status ?? {}
  const dur = fmtDuration(st.startedAt, st.finishedAt)
  return `
    <header>
      <div class="row-between">
        <strong>${esc(wr.spec?.workRef ?? '—')}</strong>
        ${badge(st.state ?? 'pending')}
      </div>
    </header>
    <div class="row" style="font-size:0.88rem;flex-wrap:wrap;gap:1rem">
      <span><span class="muted">Run ID:</span> <code>${esc(wr.name)}</code></span>
      <span><span class="muted">Trigger:</span> ${esc(wr.spec?.trigger?.type ?? '—')}</span>
      ${st.createdAt  ? `<span><span class="muted">Created:</span> ${fmtDate(st.createdAt)}</span>` : ''}
      ${st.startedAt  ? `<span><span class="muted">Started:</span> ${fmtDate(st.startedAt)}</span>` : ''}
      ${st.finishedAt ? `<span><span class="muted">Finished:</span> ${fmtDate(st.finishedAt)}</span>` : ''}
      ${st.startedAt  ? `<span><span class="muted">Duration:</span> ${dur}</span>` : ''}
    </div>
  `
}

function metricsInner(wr) {
  const s = wr.status?.summary ?? {}
  return `
    <div class="metric-card"><div class="metric-value" style="color:var(--status-succeeded)">${s.succeededSteps ?? 0}</div><div class="metric-label">Succeeded</div></div>
    <div class="metric-card"><div class="metric-value" style="color:var(--status-running)">${s.runningSteps ?? 0}</div><div class="metric-label">Running</div></div>
    <div class="metric-card"><div class="metric-value" style="color:var(--status-failed)">${s.failedSteps ?? 0}</div><div class="metric-label">Failed</div></div>
    <div class="metric-card"><div class="metric-value">${s.totalSteps ?? 0}</div><div class="metric-label">Total</div></div>
  `
}

function refreshHeader(wr) {
  const card = document.getElementById('wf-header-card')
  if (card) card.innerHTML = headerCardInner(wr)
}

function refreshSteps(wr) {
  const container = document.getElementById('steps-list')
  if (!container) return
  const steps = wr.status?.steps ?? []
  const activeFilter = container.querySelector('.step-filter-btn[aria-pressed="true"]')?.dataset.filter ?? 'all'
  const openIds = new Set(Array.from(document.querySelectorAll('.step-card.open')).map(el => el.id.replace('step-', '')))
  container.innerHTML = stepsHtml(steps, activeFilter)
  restoreOpenSteps(openIds)
  attachFilterHandlers(steps)
  attachStepToggleHandlers()
  const metricsEl = document.getElementById('wf-metrics')
  if (metricsEl) metricsEl.innerHTML = metricsInner(wr)
}

function refreshRunLogs(wr) {
  const el = document.getElementById('run-logs')
  if (el) el.innerHTML = runLogsHtml(wr.status?.logs ?? [])
}

