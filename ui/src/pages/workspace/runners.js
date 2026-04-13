// ── Workspace: Runners page ────────────────────────────────────────────────
// Two-tab view: WorkRunners tab and TaskRunners tab.
// TODO: Implement both tabs with data tables and live load gauges.
// See tasks.md → Task 007.

import { setApp, setBreadcrumb } from '../../utils.js'

export function renderRunners() {
  setBreadcrumb([{ label: 'Workspace' }, { label: 'Runners' }])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Runners</h1>
    </div>
    <div class="empty-state">
      <h3>Coming soon</h3>
      <p>The Runners page is not yet implemented. See tasks.md → Task 007.</p>
    </div>
  `)
}
