import { getLoaderMetrics } from '../api.js'
import { esc, fmtDate, setApp } from '../utils.js'
import { toastError } from '../components/toast.js'

export async function renderMetrics() {
  setApp('<p aria-busy="true">Loading metrics…</p>')
  try {
    const base = window.ORKESTER_API_BASE ?? ''
    const [res, scanMetrics] = await Promise.all([
      fetch(`${base}/v1/metrics`),
      getLoaderMetrics().catch(() => []),
    ])
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

    const scanRows = (Array.isArray(scanMetrics) ? scanMetrics : [])
      .slice().reverse()
      .map(m => `<tr>
        <td class="muted" style="font-size:0.8rem">${esc(fmtDate(m.scanned_at ?? m.scannedAt))}</td>
        <td><code style="font-size:0.8rem">${esc(m.entry_path ?? m.entryPath ?? '')}</code></td>
        <td style="text-align:center">${m.is_initial ?? m.isInitial ? '<span class="tag" style="background:#dbeafe">initial</span>' : 'poll'}</td>
        <td style="text-align:right;font-family:monospace">${m.duration_ms ?? m.durationMs ?? 0} ms</td>
        <td style="text-align:right;color:var(--status-succeeded)">${m.events_added ?? m.eventsAdded ?? 0}</td>
        <td style="text-align:right;color:var(--status-running)">${m.events_modified ?? m.eventsModified ?? 0}</td>
        <td style="text-align:right;color:var(--status-failed)">${m.events_removed ?? m.eventsRemoved ?? 0}</td>
      </tr>`)
      .join('')

    setApp(`
      <hgroup>
        <h2>Metrics</h2>
        <p>${Object.keys(data).length} server metrics</p>
      </hgroup>
      <figure>
        <table>
          <thead><tr><th>Metric</th><th style="text-align:right">Value</th></tr></thead>
          <tbody>${rows}</tbody>
        </table>
      </figure>

      <hgroup style="margin-top:2rem">
        <h3>Loader Scan History</h3>
        <p>Recent filesystem scans (newest first)</p>
      </hgroup>
      ${scanRows
        ? `<figure>
            <table>
              <thead><tr>
                <th>Time</th><th>Path</th><th>Type</th>
                <th style="text-align:right">Duration</th>
                <th style="text-align:right">Added</th>
                <th style="text-align:right">Modified</th>
                <th style="text-align:right">Removed</th>
              </tr></thead>
              <tbody>${scanRows}</tbody>
            </table>
          </figure>`
        : '<p class="muted">No scan history yet.</p>'}
    `)
  } catch (e) {
    toastError(`Failed to load metrics: ${e.message}`)
    setApp('<div class="empty-state"><p>Failed to load metrics.</p></div>')
  }
}

