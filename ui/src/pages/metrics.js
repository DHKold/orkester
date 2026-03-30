import { getMetricsSnapshot, getMetricsHistory } from '../api.js'
import { esc, setApp } from '../utils.js'
import { toastError } from '../components/toast.js'

const REFRESH_MS = 30_000
let _refreshTimer = null
let _charts = {}

// ─── Entry point ──────────────────────────────────────────────────────────────

export async function renderMetrics() {
  stopAutoRefresh()
  setApp('<p aria-busy="true">Loading metrics…</p>')
  await loadAndRender()
  startAutoRefresh()
}

// ─── Data loading ─────────────────────────────────────────────────────────────

async function loadAndRender() {
  try {
    const [snapRes, histRes] = await Promise.all([
      getMetricsSnapshot().catch(() => ({ metrics: {} })),
      getMetricsHistory().catch(() => ({ history: {} })),
    ])
    const snapshot = snapRes?.metrics ?? {}
    const history  = histRes?.history  ?? {}
    render(snapshot, history)
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
      <hgroup>
        <h2>Metrics</h2>
        <p>No metrics recorded yet.</p>
      </hgroup>`)
    return
  }

  const cards = keys.map(key => buildCard(key, snapshot[key], history[key] ?? [])).join('')
  setApp(`
    <hgroup>
      <h2>Metrics</h2>
      <p>${keys.length} metric${keys.length !== 1 ? 's' : ''} · auto-refreshes every ${REFRESH_MS / 1000}s</p>
    </hgroup>
    <div class="metrics-grid">${cards}</div>`)

  // Render charts after DOM update.
  requestAnimationFrame(() => {
    destroyAllCharts()
    keys.forEach(key => renderChart(key, history[key] ?? []))
  })
}

function buildCard(key, value, points) {
  const shortKey = key.split('.').slice(-2).join('.')
  const formatted = formatValue(value)
  const trend = trendIcon(points)
  return `
    <div class="metric-card" data-key="${esc(key)}">
      <div class="metric-card-header">
        <span class="metric-key" title="${esc(key)}">${esc(shortKey)}</span>
        <span class="metric-trend">${trend}</span>
      </div>
      <div class="metric-value">${esc(formatted)}</div>
      <canvas id="chart-${cssId(key)}" class="metric-chart" height="60"></canvas>
    </div>`
}

// ─── Charts ───────────────────────────────────────────────────────────────────

function renderChart(key, points) {
  if (!window.Chart) return
  const id = `chart-${cssId(key)}`
  const canvas = document.getElementById(id)
  if (!canvas || points.length < 2) return

  const labels = points.map(p => new Date(p.timestamp_ms).toLocaleTimeString())
  const data   = points.map(p => p.value)

  _charts[key] = new window.Chart(canvas, {
    type: 'line',
    data: {
      labels,
      datasets: [{
        data,
        borderColor:     '#3b82f6',
        backgroundColor: 'rgba(59,130,246,0.08)',
        borderWidth:     2,
        pointRadius:     0,
        fill:            true,
        tension:         0.3,
      }],
    },
    options: {
      animation:   false,
      responsive:  true,
      maintainAspectRatio: false,
      plugins: { legend: { display: false }, tooltip: { mode: 'index', intersect: false } },
      scales: {
        x: { display: false },
        y: { display: true, ticks: { maxTicksLimit: 3, font: { size: 10 } }, grid: { color: '#f1f5f9' } },
      },
    },
  })
}

function destroyAllCharts() {
  Object.values(_charts).forEach(c => c.destroy())
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
  if (typeof v !== 'number') return String(v)
  if (Number.isInteger(v)) return v.toLocaleString()
  return v.toLocaleString(undefined, { maximumFractionDigits: 2 })
}

function trendIcon(points) {
  if (points.length < 2) return ''
  const last = points[points.length - 1].value
  const prev = points[points.length - 2].value
  if (last > prev) return '<span style="color:#22c55e">▲</span>'
  if (last < prev) return '<span style="color:#ef4444">▼</span>'
  return '<span style="color:#94a3b8">–</span>'
}

function cssId(key) {
  return key.replace(/[^a-z0-9]/gi, '-')
}

