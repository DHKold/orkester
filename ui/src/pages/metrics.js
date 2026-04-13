// ── Metrics page ───────────────────────────────────────────────────────────
// Shows current metric values and Chart.js sparklines from history.
// Auto-refreshes every 30 s.
// TODO: Implement. See tasks.md → Task 014.

import { setApp, setBreadcrumb } from '../utils.js'

export function renderMetrics() {
  setBreadcrumb([{ label: 'Metrics' }])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Metrics</h1>
    </div>
    <div class="empty-state">
      <h3>Coming soon</h3>
      <p>The Metrics page is not yet implemented. See tasks.md → Task 014.</p>
    </div>
  `)
}
