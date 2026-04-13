import { esc, fmtDate, fmtDuration, badge } from '../utils.js'
import { getTaskRun } from '../api.js'

// Cache loaded TaskRunDocs to avoid redundant fetches.
export const taskRunCache = new Map()

const FILTERS = [
  { v: 'all',       l: 'All'       },
  { v: 'running',   l: 'Running'   },
  { v: 'failed',    l: 'Failed'    },
  { v: 'succeeded', l: 'Succeeded' },
]

// Full steps section HTML: filter bar + step cards.
export function stepsHtml(steps, activeFilter = 'all') {
  const counts = { all: steps.length }
  for (const s of steps) counts[s.state] = (counts[s.state] ?? 0) + 1
  const bar = FILTERS.map(({ v, l }) => {
    const a = v === activeFilter
    return `<button class="step-filter-btn outline btn-xs${a ? ' active' : ''}" aria-pressed="${a}" data-filter="${v}">${l} (${counts[v] ?? 0})</button>`
  }).join('')
  const visible = activeFilter === 'all' ? steps : steps.filter(s => s.state === activeFilter)
  const cards = visible.length
    ? visible.map(s => stepCard(s, s.state === 'failed')).join('')
    : `<p class="muted" style="font-size:0.85rem">No steps match this filter.</p>`
  return `<div class="step-filter-bar" style="display:flex;gap:0.4rem;margin-bottom:0.75rem;flex-wrap:wrap">${bar}</div><div id="step-cards">${cards}</div>`
}

function stepCard(s, autoOpen = false) {
  const st    = s.state ?? 'pending'
  const trRef = s.activeTaskRunRef ?? ''
  return `
    <div class="step-card${autoOpen ? ' open' : ''}" id="step-${esc(s.name)}">
      <div class="step-header" data-step="${esc(s.name)}" data-tr-ref="${esc(trRef)}">
        <span class="step-chevron">▶</span>
        ${badge(st)}
        <span class="step-name">${esc(s.name)}</span>
        ${trRef ? `<span class="step-meta muted" style="font-size:0.8rem">${esc(trRef.substring(0,12))}…</span>` : ''}
        <span class="step-meta muted" style="font-size:0.8rem">attempts: ${s.attempts ?? 0}</span>
      </div>
      <div class="step-body">
        <p class="muted" style="font-size:0.85rem">Request: <code>${esc(s.taskRunRequestRef ?? '—')}</code></p>
        <div class="step-tr-details"></div>
      </div>
    </div>`
}

export function renderTaskRunDetails(tr) {
  const sp = tr?.spec ?? {}
  const s  = tr?.status ?? {}
  const inputs  = s.inputs  ?? {}
  const outputs = s.outputs ?? {}
  const logs    = s.logsRef ?? null
  const iKeys = Object.keys(inputs)
  const oKeys = Object.keys(outputs)
  const dur = fmtDuration(s.startedAt ?? s.started_at, s.finishedAt ?? s.finished_at)

  let html = `<div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(180px,1fr));gap:0.4rem;margin-bottom:0.5rem;font-size:0.82rem">
    ${sp.taskRef   || sp.task_ref   ? `<div><span class="muted">Task:</span> <code>${esc(sp.taskRef ?? sp.task_ref)}</code></div>` : ''}
    ${sp.stepName  || sp.step_name  ? `<div><span class="muted">Step:</span> <strong>${esc(sp.stepName ?? sp.step_name)}</strong></div>` : ''}
    <div><span class="muted">Attempt:</span> ${esc(sp.attempt ?? 1)}</div>
    ${s.startedAt  || s.started_at  ? `<div><span class="muted">Started:</span>  ${fmtDate(s.startedAt  ?? s.started_at)}</div>`  : ''}
    ${s.finishedAt || s.finished_at ? `<div><span class="muted">Finished:</span> ${fmtDate(s.finishedAt ?? s.finished_at)}</div>` : ''}
    ${s.startedAt  || s.started_at  ? `<div><span class="muted">Duration:</span> <strong>${dur}</strong></div>` : ''}
  </div>`

  if (iKeys.length) html += tableDetails('Inputs',  iKeys, inputs)
  if (oKeys.length) html += tableDetails('Outputs', oKeys, outputs)

  if (logs) {
    const hasOut = logs.stdout?.trim(); const hasErr = logs.stderr?.trim()
    if (hasOut || hasErr) html += logBlock(hasOut ? logs.stdout : null, hasErr ? logs.stderr : null)
  }

  if (!iKeys.length && !oKeys.length && !logs) return html + '<p class="muted" style="font-size:0.85rem">No details available.</p>'
  return html
}

function tableDetails(title, keys, map) {
  const rows = keys.map(k => `<tr><td><code>${esc(k)}</code></td><td>${esc(JSON.stringify(map[k]))}</td></tr>`).join('')
  return `<details open style="margin:0.5rem 0"><summary style="font-size:0.85rem"><strong>${title}</strong> <span class="muted">(${keys.length})</span></summary>
    <table style="font-size:0.82rem;margin-top:0.25rem"><thead><tr><th>Name</th><th>Value</th></tr></thead><tbody>${rows}</tbody></table></details>`
}

function logBlock(stdout, stderr) {
  const pre = (label, text, bg) =>
    `<pre style="font-size:0.78rem;max-height:200px;overflow-y:auto;background:${bg};padding:0.5rem;border-radius:4px;margin:0.25rem 0"><strong>${label}</strong>\n${esc(text)}</pre>`
  return `<details open style="margin:0.5rem 0"><summary style="font-size:0.85rem"><strong>Logs</strong></summary>
    ${stdout ? pre('stdout', stdout, '#f0f4f8') : ''}${stderr ? pre('stderr', stderr, '#fff5f5') : ''}</details>`
}

// WorkRun-level structured run log.
export function runLogsHtml(logs = []) {
  if (!logs.length) return '<p class="muted" style="font-size:0.85rem">No run logs yet.</p>'
  const col = { info: 'var(--pico-muted-color)', warn: 'var(--status-running)', error: 'var(--status-failed)' }
  const rows = logs.map(e =>
    `<div style="display:grid;grid-template-columns:6rem 3.5rem 1fr;gap:0.5rem;padding:0.2rem 0;border-bottom:1px solid var(--pico-muted-border-color)">
      <span class="muted" style="font-size:0.78rem">${esc(e.ts?.substring(11, 19) ?? '')}</span>
      <span style="color:${col[e.level] ?? 'inherit'};font-weight:600;font-size:0.78rem">${esc((e.level ?? '').toUpperCase())}</span>
      <span style="font-size:0.8rem">${esc(e.message ?? '')}</span>
    </div>`
  ).join('')
  return `<div style="font-family:monospace">${rows}</div>`
}

// After re-rendering the steps list, restore previously open cards from cache.
export function restoreOpenSteps(openIds) {
  openIds.forEach(sid => {
    const card = document.getElementById(`step-${sid}`)
    if (!card) return
    card.classList.add('open')
    const trRef = card.querySelector('.step-header')?.dataset?.trRef
    if (trRef && taskRunCache.has(trRef)) {
      const det = card.querySelector('.step-tr-details')
      if (det) det.innerHTML = renderTaskRunDetails(taskRunCache.get(trRef))
    }
  })
}

export function attachStepToggleHandlers() {
  document.querySelectorAll('.step-header').forEach(header => {
    header.addEventListener('click', async () => {
      const card = header.closest('.step-card')
      card.classList.toggle('open')
      if (!card.classList.contains('open')) return
      const trRef = header.dataset.trRef
      if (!trRef) return
      const det = card.querySelector('.step-tr-details')
      if (!det) return
      if (taskRunCache.has(trRef)) { det.innerHTML = renderTaskRunDetails(taskRunCache.get(trRef)); return }
      det.innerHTML = '<p class="muted" aria-busy="true" style="font-size:0.85rem">Loading…</p>'
      try {
        const tr = await getTaskRun(trRef)
        taskRunCache.set(trRef, tr)
        det.innerHTML = renderTaskRunDetails(tr)
      } catch (e) {
        det.innerHTML = `<p class="muted" style="font-size:0.85rem">Could not load task run: ${esc(e.message)}</p>`
      }
    })
  })
}

// Wire filter-button clicks; re-renders only #step-cards.
export function attachFilterHandlers(steps) {
  document.querySelectorAll('.step-filter-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      document.querySelectorAll('.step-filter-btn').forEach(b => { b.classList.remove('active'); b.setAttribute('aria-pressed', 'false') })
      btn.classList.add('active'); btn.setAttribute('aria-pressed', 'true')
      const cardsDiv = document.getElementById('step-cards')
      if (!cardsDiv) return
      const f = btn.dataset.filter
      const visible = f === 'all' ? steps : steps.filter(s => s.state === f)
      cardsDiv.innerHTML = visible.length
        ? visible.map(s => stepCard(s, s.state === 'failed')).join('')
        : `<p class="muted" style="font-size:0.85rem">No steps match this filter.</p>`
      attachStepToggleHandlers()
    })
  })
}

export function scrollToStep(stepId) {
  const el = document.getElementById(`step-${stepId}`)
  if (el) { el.classList.add('open'); el.scrollIntoView({ behavior: 'smooth', block: 'nearest' }) }
}
