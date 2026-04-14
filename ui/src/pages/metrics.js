// ── Metrics page ───────────────────────────────────────────────────────────
// Shows current metric values and Chart.js sparklines from history.
// Auto-refreshes every 30 s.

import { getMetricsSnapshot, getMetricsHistory }  from '../api.js'
import { setApp, setBreadcrumb, esc }              from '../utils.js'
import { setCleanup }                              from '../router.js'
import { toastError }                             from '../components/toast.js'

const REFRESH_MS = 30_000
let _timer  = null
let _charts = {}

// ── Entry point ────────────────────────────────────────────────────────────

export async function renderMetrics() {
  stopRefresh()
  setBreadcrumb([{ label: 'Metrics' }])
  setApp(`<div class="loading-state" aria-busy="true">Loading metrics…</div>`)
  await loadAndRender()
  _timer = setInterval(loadAndRender, REFRESH_MS)
  setCleanup(() => { stopRefresh(); destroyCharts() })
}

// ── Data loading ───────────────────────────────────────────────────────────

async function loadAndRender() {
  try {
    const [snapRes, histRes] = await Promise.all([
      getMetricsSnapshot().catch(() => ({})),
      getMetricsHistory().catch(() => ({})),
    ])
    render(snapRes?.metrics ?? {}, histRes?.metrics ?? histRes?.history ?? {})
  } catch (e) {
    toastError(`Failed to load metrics: ${e.message}`)
    setApp(`<div class="empty-state"><h3>Error</h3><p>${esc(e.message)}</p></div>`)
  }
}

// ── Rendering ─────────────────────────────────────────────────────────────

function render(snapshot, history) {
  const keys = Object.keys(snapshot).sort()

  setBreadcrumb([{ label: 'Metrics' }])

  if (keys.length === 0) {
    setApp(`
      <div class="page-title-row">
        <h1 class="page-title">Metrics</h1>
      </div>
      <div class="empty-state">
        <h3>No metrics yet</h3>
        <p>No metric values have been recorded. Metrics appear once the server starts processing requests.</p>
      </div>`)
    return
  }

  const histKeys = keys.filter(k => (history[k] ?? []).length >= 2)

  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Metrics</h1>
      <div class="page-title-actions" style="font-size:0.8rem;color:var(--text-3)">
        Auto-refresh every ${REFRESH_MS / 1000}s
      </div>
    </div>

    <div class="form-section-header">Current Values</div>
    <div class="metrics-snap-grid">
      ${keys.map(k => snapshotCard(k, snapshot[k], history[k] ?? [])).join('')}
    </div>

    ${histKeys.length > 0 ? `
      <div class="form-section-header" style="margin-top:2rem">History</div>
      <div class="metrics-history-grid">
        ${histKeys.map(k => historyCard(k, snapshot[k], history[k])).join('')}
      </div>` : ''}
  `)

  // Mount charts after DOM is ready
  requestAnimationFrame(() => {
    destroyCharts()
    histKeys.forEach(k => mountChart(k, history[k] ?? []))
  })
}

// ── Card builders ──────────────────────────────────────────────────────────

function snapshotCard(key, value, points) {
  return `
    <div class="metrics-snap-card">
      <div class="metrics-snap-value">${esc(fmtValue(value))}</div>
      <div class="metrics-snap-key" title="${esc(key)}">${esc(shortLabel(key))}</div>
      ${trendHtml(points)}
    </div>`
}

function historyCard(key, value, points) {
  const id = chartId(key)
  return `
    <div class="metrics-history-card">
      <div class="metrics-history-header">
        <span class="metrics-history-key" title="${esc(key)}">${esc(shortLabel(key))}</span>
        <span class="metrics-history-value">${esc(fmtValue(value))}</span>
        ${trendHtml(points)}
      </div>
      <div class="metrics-chart-wrap">
        <canvas id="${id}"></canvas>
      </div>
    </div>`
}

// ── Chart.js integration ───────────────────────────────────────────────────

function mountChart(key, points) {
  if (!window.Chart || points.length < 2) return
  const canvas = document.getElementById(chartId(key))
  if (!canvas) return

  _charts[key] = new window.Chart(canvas, {
    type: 'line',
    data: {
      labels:   points.map(p => fmtTime(p.timestamp_ms)),
      datasets: [{
        data:            points.map(p => p.value),
        borderColor:     '#3b82f6',
        backgroundColor: 'rgba(59,130,246,0.08)',
        borderWidth:     1.5,
        pointRadius:     0,
        fill:            true,
        tension:         0.3,
      }],
    },
    options: {
      animation:           false,
      responsive:          true,
      maintainAspectRatio: false,
      plugins: {
        legend:  { display: false },
        tooltip: {
          enabled:   true,
          mode:      'index',
          intersect: false,
          callbacks: { label: ctx => fmtValue(ctx.parsed.y) },
        },
      },
      scales: {
        x: { display: false },
        y: {
          display:  true,
          position: 'right',
          ticks:    { maxTicksLimit: 3, font: { size: 9 }, color: 'var(--text-3)' },
          grid:     { color: 'rgba(0,0,0,0.04)' },
        },
      },
    },
  })
}

function destroyCharts() {
  Object.values(_charts).forEach(c => { try { c.destroy() } catch (_) {} })
  _charts = {}
}

// ── Auto-refresh ───────────────────────────────────────────────────────────

function stopRefresh() {
  if (_timer) { clearInterval(_timer); _timer = null }
}

// ── Helpers ────────────────────────────────────────────────────────────────

function fmtValue(v) {
  if (typeof v !== 'number') return String(v ?? '—')
  if (Number.isInteger(v))   return v.toLocaleString()
  return v.toLocaleString(undefined, { minimumFractionDigits: 0, maximumFractionDigits: 3 })
}

/** Show last 2 dot/slash segments so "workaholic.work_runs.running" → "work_runs.running" */
function shortLabel(key) {
  const sep   = key.includes('.') ? '.' : '/'
  const parts = key.split(sep)
  return parts.length > 2 ? parts.slice(-2).join(sep) : key
}

function trendHtml(points) {
  if (points.length < 2) return ''
  const last = points[points.length - 1]?.value
  const prev = points[points.length - 2]?.value
  if (last == null || prev == null) return ''
  if (last > prev) return '<span class="trend-up">▲</span>'
  if (last < prev) return '<span class="trend-dn">▼</span>'
  return '<span class="trend-eq">—</span>'
}

function fmtTime(ms) {
  return new Date(ms).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

function chartId(key) {
  return `mc-${key.replace(/[^a-z0-9]/gi, '-')}`
}

