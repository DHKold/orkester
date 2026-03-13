import { setApp } from '../utils.js'
import { toastError } from '../components/toast.js'

export async function renderMetrics() {
  setApp('<p aria-busy="true">Loading metrics…</p>')
  try {
    const base = window.ORKESTER_API_BASE ?? ''
    const res  = await fetch(`${base}/v1/metrics`)
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    const data = await res.json()

    const rows = Object.entries(data)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, val]) => `
        <tr>
          <td style="font-family:monospace">${key}</td>
          <td style="text-align:right;font-family:monospace;font-weight:600">${val}</td>
        </tr>`)
      .join('')

    setApp(`
      <hgroup>
        <h2>Metrics</h2>
        <p>${Object.keys(data).length} metrics</p>
      </hgroup>
      <figure>
        <table>
          <thead><tr><th>Metric</th><th style="text-align:right">Value</th></tr></thead>
          <tbody>${rows}</tbody>
        </table>
      </figure>
    `)
  } catch (e) {
    toastError(`Failed to load metrics: ${e.message}`)
    setApp('<div class="empty-state"><p>Failed to load metrics.</p></div>')
  }
}

