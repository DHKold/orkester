import { getMetricsSnapshot, getMetricsHistory } from '../api.js'
import { esc, setApp } from '../utils.js'
import { toastError } from '../components/toast.js'
import { setCleanup } from '../router.js'

const REFRESH_MS = 30_000
let _refreshTimer = null
let _charts = {}

// ─── Entry point ──────────────────────────────────────────────────────────────

export async function renderMetrics() {
  stopAutoRefresh()
  setApp('<p aria-busy="true">Loading metrics…</p>')
  await loadAndRender()
  startAutoRefresh()
  setCleanup(() => { stopAutoRefresh(); destroyAllCharts() })
}

// ─── Data loading ─────────────────────────────────────────────────────────────

async function loadAndRender() {
  try {
    const [snapRes, histRes] = await Promise.all([
      getMetricsSnapshot().catch(() => ({ metrics: {} })),
      getMetricsHistory().catch(() => ({ history: {} })),
    ])
    render(snapRes?.metrics ?? {}, histRes?.history ?? {})
  } catch (e) {
    toastError(`Failed to load metrics: ${e.message}`)
    setApp('<div class="empty-state"><p>Failed to load metrics.</p></div>')
  }
}

// ─── Rendering ────────────────────────────────────────────────────────────────

function render(snapshot, history) {
  const keys = Object.keys(snapshot).sort()
  if (keys.length === 0) {
    setApp(`
      <hgroup><h2>Metrics</h2><p>No metrics recorded yet.</p></hgroup>`)
    return
  }

  setApp(`
    <div class="row-between" style="margin-bottom:1.25rem">
      <hgroup style="margin:0">
        <h2 style="margin:0">Metrics</h2>
        <p style="margin:0">${keys.length} metric${keys.length !== 1 ? 's' : ''} · auto-refreshes every ${REFRESH_MS / 1000}s</p>
      </hgroup>
    </div>

    <section class="metrics-snapshot-section">
      <p class="metrics-section-title">Current Values</p>
      <div class="snap-grid">${buildSnapshotCells(keys, snapshot)}</div>
    </section>

    <section class="metrics-history-section">
      <p class="metrics-section-title">History</p>
      <div class="metrics-history-grid">${buildHistoryCards(keys, snapshot, history)}</div>
    </section>`)

  requestAnimationFrame(() => {
    destroyAllCharts()
    keys.forEach(key => renderChart(key, history[key] ?? []))
  })
}

function buildSnapshotCells(keys, snapshot) {
  return keys.map(key => `
    <div class="snap-cell">
      <span class="snap-value">${esc(formatValue(snapshot[key]))}</span>
      <span class="snap-key" title="${esc(key)}">${esc(labelFor(key))}</span>
    </div>`).join('')
}

function buildHistoryCards(keys, snapshot, history) {
  return keys.map(key => {
    const pts = history[key] ?? []
    const canvasId = `chart-${cssId(key)}`
    const chart = pts.length >= 2
      ? `<div class="metric-chart-wrap"><canvas id="${canvasId}"></canvas></div>`
      : `<div class="metric-chart-empty">no history yet</div>`
    return `
      <div class="metric-history-card">
        <div class="metric-history-header">
          <span class="metric-history-key" title="${esc(key)}">${esc(labelFor(key))}</span>
          ${trendIcon(pts)}
        </div>
        ${chart}
        <div class="metric-history-value">${esc(formatValue(snapshot[key]))}</div>
      </div>`
  }).join('')
}

// ─── Charts ───────────────────────────────────────────────────────────────────

function renderChart(key, points) {
  if (!window.Chart || points.length < 2) return
  const canvas = document.getElementById(`chart-${cssId(key)}`)
  if (!canvas) return

  _charts[key] = new window.Chart(canvas, {
    type: 'line',
    data: {
      labels: points.map(p => fmtTime(p.timestamp_ms)),
      datasets: [{
        data:            points.map(p => p.value),
        borderColor:     '#3b82f6',
        backgroundColor: 'rgba(59,130,246,0.07)',
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
        tooltip: { enabled: true, mode: 'index', intersect: false },
      },
      scales: {
        x: { display: false },
        y: {
          display:  true,
          position: 'right',
          ticks:    { maxTicksLimit: 3, font: { size: 9 } },
          grid:     { color: 'rgba(0,0,0,0.04)' },
        },
      },
    },
  })
}

function destroyAllCharts() {
  Object.values(_charts).forEach(c => { try { c.destroy() } catch (_) {} })
  _charts = {}
}

// ─── Auto-refresh ─────────────────────────────────────────────────────────────

function startAutoRefresh() {
  _refreshTimer = setInterval(loadAndRender, REFRESH_MS)
}

function stopAutoRefresh() {
  if (_refreshTimer) { clearInterval(_refreshTimer); _refreshTimer = null }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function formatValue(v) {
  if (typeof v !== 'number') return String(v ?? '–')
  if (Number.isInteger(v))   return v.toLocaleString()
  return v.toLocaleString(undefined, { minimumFractionDigits: 0, maximumFractionDigits: 3 })
}

function labelFor(key) {
  const parts = key.split('.')
  return parts.length > 2 ? parts.slice(-2).join('.') : key
}

function trendIcon(points) {
  if (points.length < 2) return ''
  const last = points[points.length - 1]?.value
  const prev = points[points.length - 2]?.value
  if (last == null || prev == null) return ''
  if (last > prev) return '<span class="trend-up">▲</span>'
  if (last < prev) return '<span class="trend-dn">▼</span>'
  return '<span class="trend-eq">–</span>'
}

function fmtTime(ms) {
  return new Date(ms).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

function cssId(key) {
  return key.replace(/[^a-z0-9]/gi, '-')
}
