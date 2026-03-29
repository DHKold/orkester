import { getWorkRun, cancelWorkRun, listWorks } from '../api.js'
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
  let wr

  try {
    wr = await getWorkRun(id)
  } catch (e) {
    toastError(`Workflow not found: ${e.message}`)
    setApp(`${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns,href:`#/namespaces/${encodeURIComponent(ns)}`},{label:'Workflows',href:`#/namespaces/${encodeURIComponent(ns)}/workflows`}])}<div class="empty-state"><p>Workflow run not found.</p></div>`)
    return
  }

  renderDetail(ns, wr)
  const dagContainer = document.getElementById('dag-container')
  if (dagContainer) {
    const stepStates = buildStepStateMap(wr)
    cy = renderDag(dagContainer, buildWorkFromRun(wr), stepStates, (stepId) => scrollToStep(stepId))
  }

  const state = wr.status?.state ?? 'pending'
  if (!TERMINAL.has(state)) {
    intervalId = setInterval(async () => {
      try {
        wr = await getWorkRun(id)
        refreshHeader(wr)
        const stepStates = buildStepStateMap(wr)
        updateDagColors(cy, stepStates)
        refreshSteps(wr)
        if (TERMINAL.has(wr.status?.state ?? '')) {
          clearInterval(intervalId); intervalId = null
        }
      } catch (_) {}
    }, REFRESH_MS)
    setCleanup(() => { if (intervalId) clearInterval(intervalId) })
  }
}

// ── Data adapters ─────────────────────────────────────────────────────────────

function buildStepStateMap(wr) {
  const steps = wr.status?.steps ?? []
  const map = {}
  for (const s of steps) { map[s.name] = { status: s.state, attempts: s.attempts } }
  return map
}

function buildWorkFromRun(wr) {
  // Reconstruct a minimal Work-like shape from the WorkRunRequest steps for DAG rendering.
  const steps = (wr.status?.steps ?? []).map(s => ({
    id: s.name, name: s.name, depends_on: []
  }))
  return { spec: { steps } }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

function renderDetail(ns, wr) {
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
    <div class="row-between" style="margin-bottom:1rem">
      <div></div>
      <div class="row" style="gap:0.5rem">
        ${!TERMINAL.has(status)
          ? `<button class="outline btn-xs" id="btn-cancel">Cancel</button>`
          : ''}
      </div>
    </div>
    <article id="wf-header-card">${headerCardInner(wr)}</article>
    <div class="metrics-grid" id="wf-metrics">${metricsInner(wr)}</div>
    <h4>Execution Graph</h4>
    <div id="dag-container"></div>
    <h4>Steps</h4>
    <div id="steps-list">${stepsInner(steps)}</div>
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
}

function headerCardInner(wr) {
  const st  = wr.status ?? {}
  const dur = fmtDuration(st.started_at, st.finished_at)
  return `
    <header>
      <div class="row-between">
        <strong>${esc(wr.spec?.work_ref ?? '—')}</strong>
        ${badge(st.state ?? 'pending')}
      </div>
    </header>
    <div class="row" style="font-size:0.88rem;flex-wrap:wrap;gap:1rem">
      <span><span class="muted">Run ID:</span> <code>${esc(wr.name)}</code></span>
      <span><span class="muted">Trigger:</span> ${esc(wr.spec?.trigger?.type ?? '—')}</span>
      ${st.created_at  ? `<span><span class="muted">Created:</span> ${fmtDate(st.created_at)}</span>` : ''}
      ${st.started_at  ? `<span><span class="muted">Started:</span> ${fmtDate(st.started_at)}</span>` : ''}
      ${st.finished_at ? `<span><span class="muted">Finished:</span> ${fmtDate(st.finished_at)}</span>` : ''}
      ${st.started_at  ? `<span><span class="muted">Duration:</span> ${dur}</span>` : ''}
    </div>
  `
}

function metricsInner(wr) {
  const s = wr.status?.summary ?? {}
  return `
    <div class="metric-card"><div class="metric-value" style="color:var(--status-succeeded)">${s.succeeded_steps ?? 0}</div><div class="metric-label">Succeeded</div></div>
    <div class="metric-card"><div class="metric-value" style="color:var(--status-running)">${s.running_steps ?? 0}</div><div class="metric-label">Running</div></div>
    <div class="metric-card"><div class="metric-value" style="color:var(--status-failed)">${s.failed_steps ?? 0}</div><div class="metric-label">Failed</div></div>
    <div class="metric-card"><div class="metric-value">${s.total_steps ?? 0}</div><div class="metric-label">Total</div></div>
  `
}

function stepsInner(steps) {
  if (!steps.length) return '<p class="muted">No steps yet.</p>'
  return steps.map(s => stepCard(s)).join('')
}

function stepCard(s) {
  const status = s.state ?? 'pending'
  const trRef  = s.active_task_run_ref ?? ''
  return `
    <div class="step-card" id="step-${esc(s.name)}">
      <div class="step-header" data-step="${esc(s.name)}">
        <span class="step-chevron">▶</span>
        ${badge(status)}
        <span class="step-name">${esc(s.name)}</span>
        ${trRef ? `<span class="step-meta muted" style="font-size:0.8rem">${esc(trRef.substring(0,12))}…</span>` : ''}
        <span class="step-meta muted" style="font-size:0.8rem">attempts: ${s.attempts ?? 0}</span>
      </div>
      <div class="step-body">
        <p class="muted">Task run request: <code>${esc(s.task_run_request_ref ?? '—')}</code></p>
      </div>
    </div>
  `
}

function refreshHeader(wr) {
  const card = document.getElementById('wf-header-card')
  if (card) card.innerHTML = headerCardInner(wr)
}

function refreshSteps(wr) {
  const container = document.getElementById('steps-list')
  if (!container) return
  const openIds = new Set(
    Array.from(document.querySelectorAll('.step-card.open')).map(el => el.id.replace('step-', ''))
  )
  container.innerHTML = stepsInner(wr.status?.steps ?? [])
  openIds.forEach(sid => {
    const card = document.getElementById(`step-${sid}`)
    if (card) card.classList.add('open')
  })
  attachStepToggleHandlers()
  const metricsEl = document.getElementById('wf-metrics')
  if (metricsEl) metricsEl.innerHTML = metricsInner(wr)
}

function attachStepToggleHandlers() {
  document.querySelectorAll('.step-header').forEach(header => {
    header.addEventListener('click', () => header.closest('.step-card').classList.toggle('open'))
  })
}

function scrollToStep(stepId) {
  const el = document.getElementById(`step-${stepId}`)
  if (el) { el.classList.add('open'); el.scrollIntoView({ behavior: 'smooth', block: 'nearest' }) }
}

