import { getWorkRun, cancelWorkRun, listWorks, getTaskRun } from '../api.js'
import { esc, fmtDate, fmtDuration, badge, setApp, breadcrumb } from '../utils.js'
import { toastError, toastSuccess } from '../components/toast.js'
import { renderDag, updateDagColors } from '../components/dag.js'
import { setCleanup, navigate } from '../router.js'

const TERMINAL = new Set(['succeeded', 'failed', 'cancelled'])
const REFRESH_MS = 3000

// Cache loaded TaskRunDocs by their name to avoid redundant fetches.
const taskRunCache = new Map()

export async function renderWorkflow({ ns, id }) {
  setApp(`
    ${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns,href:`#/namespaces/${encodeURIComponent(ns)}`},{label:'Workflows'}])}
    <p aria-busy="true">Loading workflow…</p>
  `)

  let cy = null
  let intervalId = null
  let wr
  let workDef = null

  try {
    wr = await getWorkRun(id)
  } catch (e) {
    toastError(`Workflow not found: ${e.message}`)
    setApp(`${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns,href:`#/namespaces/${encodeURIComponent(ns)}`},{label:'Workflows',href:`#/namespaces/${encodeURIComponent(ns)}/workflows`}])}<div class="empty-state"><p>Workflow run not found.</p></div>`)
    return
  }

  // Try to load the Work definition for accurate dependency graph rendering.
  const workRef = wr.spec?.workRef ?? ''
  const [wns, wname] = workRef.includes('/') ? workRef.split('/') : [ns, workRef]
  try {
    const { works } = await listWorks(wns)
    workDef = (works ?? []).find(w => w.name === wname) ?? null
  } catch (_) {}

  renderDetail(ns, wr, workDef)
  const dagContainer = document.getElementById('dag-container')
  if (dagContainer) {
    const stepStates = buildStepStateMap(wr)
    cy = renderDag(dagContainer, workDef ?? buildWorkFromRun(wr), stepStates, (stepId) => scrollToStep(stepId))
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
  // Reconstruct a minimal Work-like shape from WorkRun steps (no dependency info).
  const steps = (wr.status?.steps ?? []).map(s => ({
    name: s.name, depends_on: []
  }))
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

function stepsInner(steps) {
  if (!steps.length) return '<p class="muted">No steps yet.</p>'
  return steps.map(s => stepCard(s)).join('')
}

function stepCard(s) {
  const status = s.state ?? 'pending'
  const trRef  = s.activeTaskRunRef ?? ''
  return `
    <div class="step-card" id="step-${esc(s.name)}">
      <div class="step-header" data-step="${esc(s.name)}" data-tr-ref="${esc(trRef)}">
        <span class="step-chevron">▶</span>
        ${badge(status)}
        <span class="step-name">${esc(s.name)}</span>
        ${trRef ? `<span class="step-meta muted" style="font-size:0.8rem">${esc(trRef.substring(0,12))}…</span>` : ''}
        <span class="step-meta muted" style="font-size:0.8rem">attempts: ${s.attempts ?? 0}</span>
      </div>
      <div class="step-body">
        <p class="muted" style="font-size:0.85rem">Request: <code>${esc(s.taskRunRequestRef ?? '—')}</code></p>
        <div class="step-tr-details"></div>
      </div>
    </div>
  `
}

function renderTaskRunDetails(tr) {
  const sp      = tr?.spec    ?? {}
  const s       = tr?.status  ?? {}
  const inputs  = s.inputs    ?? {}
  const outputs = s.outputs   ?? {}
  const logs    = s.logsRef   ?? null
  const inputKeys  = Object.keys(inputs)
  const outputKeys = Object.keys(outputs)

  const dur = fmtDuration(s.startedAt ?? s.started_at, s.finishedAt ?? s.finished_at)

  // ── Meta header ────────────────────────────────────────────────────────
  let html = `
    <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(180px,1fr));gap:0.4rem;margin-bottom:0.5rem;font-size:0.82rem">
      ${sp.taskRef || sp.task_ref ? `<div><span class="muted">Task:</span> <code>${esc(sp.taskRef ?? sp.task_ref)}</code></div>` : ''}
      ${sp.stepName || sp.step_name ? `<div><span class="muted">Step:</span> <strong>${esc(sp.stepName ?? sp.step_name)}</strong></div>` : ''}
      <div><span class="muted">Attempt:</span> ${esc(sp.attempt ?? 1)}</div>
      ${s.startedAt || s.started_at ? `<div><span class="muted">Started:</span> ${fmtDate(s.startedAt ?? s.started_at)}</div>` : ''}
      ${s.finishedAt || s.finished_at ? `<div><span class="muted">Finished:</span> ${fmtDate(s.finishedAt ?? s.finished_at)}</div>` : ''}
      ${s.startedAt || s.started_at ? `<div><span class="muted">Duration:</span> <strong>${dur}</strong></div>` : ''}
    </div>`

  // ── Inputs ─────────────────────────────────────────────────────────────
  if (inputKeys.length > 0) {
    html += `
      <details open style="margin:0.5rem 0">
        <summary style="font-size:0.85rem"><strong>Inputs</strong> <span class="muted">(${inputKeys.length})</span></summary>
        <table style="font-size:0.82rem;margin-top:0.25rem">
          <thead><tr><th>Name</th><th>Value</th></tr></thead>
          <tbody>${inputKeys.map(k => `<tr><td><code>${esc(k)}</code></td><td>${esc(JSON.stringify(inputs[k]))}</td></tr>`).join('')}</tbody>
        </table>
      </details>`
  }

  // ── Outputs ────────────────────────────────────────────────────────────
  if (outputKeys.length > 0) {
    html += `
      <details open style="margin:0.5rem 0">
        <summary style="font-size:0.85rem"><strong>Outputs</strong> <span class="muted">(${outputKeys.length})</span></summary>
        <table style="font-size:0.82rem;margin-top:0.25rem">
          <thead><tr><th>Name</th><th>Value</th></tr></thead>
          <tbody>${outputKeys.map(k => `<tr><td><code>${esc(k)}</code></td><td>${esc(JSON.stringify(outputs[k]))}</td></tr>`).join('')}</tbody>
        </table>
      </details>`
  }

  // ── Logs ───────────────────────────────────────────────────────────────
  if (logs) {
    const hasStdout = logs.stdout && logs.stdout.trim()
    const hasStderr = logs.stderr && logs.stderr.trim()
    if (hasStdout || hasStderr) {
      html += `
        <details open style="margin:0.5rem 0">
          <summary style="font-size:0.85rem"><strong>Logs</strong></summary>
          ${hasStdout ? `<pre style="font-size:0.78rem;max-height:200px;overflow-y:auto;background:#f0f4f8;padding:0.5rem;border-radius:4px;margin:0.25rem 0"><strong>stdout</strong>\n${esc(logs.stdout)}</pre>` : ''}
          ${hasStderr ? `<pre style="font-size:0.78rem;max-height:200px;overflow-y:auto;background:#fff5f5;padding:0.5rem;border-radius:4px;margin:0.25rem 0"><strong>stderr</strong>\n${esc(logs.stderr)}</pre>` : ''}
        </details>`
    }
  }

  if (!html.trim() || (!inputKeys.length && !outputKeys.length && !logs)) {
    return (html || '') + '<p class="muted" style="font-size:0.85rem">No details available.</p>'
  }
  return html
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
    if (!card) return
    card.classList.add('open')
    const trRef = card.querySelector('.step-header')?.dataset?.trRef
    if (trRef && taskRunCache.has(trRef)) {
      const detailsEl = card.querySelector('.step-tr-details')
      if (detailsEl) detailsEl.innerHTML = renderTaskRunDetails(taskRunCache.get(trRef))
    }
  })
  attachStepToggleHandlers()
  const metricsEl = document.getElementById('wf-metrics')
  if (metricsEl) metricsEl.innerHTML = metricsInner(wr)
}

function attachStepToggleHandlers() {
  document.querySelectorAll('.step-header').forEach(header => {
    header.addEventListener('click', async () => {
      const card = header.closest('.step-card')
      card.classList.toggle('open')
      if (!card.classList.contains('open')) return
      const trRef = header.dataset.trRef
      if (!trRef) return
      const detailsEl = card.querySelector('.step-tr-details')
      if (!detailsEl) return
      if (taskRunCache.has(trRef)) {
        detailsEl.innerHTML = renderTaskRunDetails(taskRunCache.get(trRef))
        return
      }
      detailsEl.innerHTML = '<p class="muted" aria-busy="true" style="font-size:0.85rem">Loading…</p>'
      try {
        const tr = await getTaskRun(trRef)
        taskRunCache.set(trRef, tr)
        detailsEl.innerHTML = renderTaskRunDetails(tr)
      } catch (e) {
        detailsEl.innerHTML = `<p class="muted" style="font-size:0.85rem">Could not load task run: ${esc(e.message)}</p>`
      }
    })
  })
}

function scrollToStep(stepId) {
  const el = document.getElementById(`step-${stepId}`)
  if (el) { el.classList.add('open'); el.scrollIntoView({ behavior: 'smooth', block: 'nearest' }) }
}

