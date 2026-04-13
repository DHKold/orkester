// ── Workspace: Work Runs list page ─────────────────────────────────────────
// Paginated, searchable list of WorkRuns scoped to the active namespace.
// Auto-refreshes every 30 s. Global action: "New Work Run" modal.
// TODO: Implement. See tasks.md → Task 010.

import { setApp, setBreadcrumb } from '../../utils.js'

export function renderWorkRuns() {
  setBreadcrumb([{ label: 'Workspace' }, { label: 'Work Runs' }])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Work Runs</h1>
    </div>
    <div class="empty-state">
      <h3>Coming soon</h3>
      <p>The Work Runs list is not yet implemented. See tasks.md → Task 010.</p>
    </div>
  `)
}
