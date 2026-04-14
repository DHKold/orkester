// ── Dashboard page ─────────────────────────────────────────────────────────
// TODO: Implement widget grid with localStorage layout persistence.
// See tasks.md → Task 004 for full spec.

import { setApp, setBreadcrumb } from '../utils.js'

export function renderDashboard() {
  setBreadcrumb([{ label: 'Dashboard' }])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Dashboard</h1>
    </div>
    <div class="empty-state">
      <h3>Coming soon</h3>
      <p>The Dashboard widget grid is not yet implemented. See tasks.md → Task 004.</p>
    </div>
  `)
}
