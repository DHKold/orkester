// ── Workspace: Crons list page ─────────────────────────────────────────────
// Paginated, searchable list of Crons scoped to the active namespace.
// Global action: Create Cron. Row actions: Edit, Toggle enabled, Delete.
// TODO: Implement. See tasks.md → Task 012.

import { setApp, setBreadcrumb } from '../../utils.js'

export function renderCrons() {
  setBreadcrumb([{ label: 'Workspace' }, { label: 'Crons' }])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Crons</h1>
    </div>
    <div class="empty-state">
      <h3>Coming soon</h3>
      <p>The Crons list is not yet implemented. See tasks.md → Task 012.</p>
    </div>
  `)
}
