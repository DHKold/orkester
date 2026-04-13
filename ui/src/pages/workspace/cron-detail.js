// ── Workspace: Cron detail page ────────────────────────────────────────────
// Shows spec, all status fields (last_scheduled_time, next_scheduled_time,
// last_run_status, consecutive_failures, run_count), and the list of
// WorkRuns triggered by this Cron.
// TODO: Implement. See tasks.md → Task 013.

import { setApp, setBreadcrumb, esc } from '../../utils.js'

export function renderCronDetail({ name } = {}) {
  setBreadcrumb([
    { label: 'Workspace' },
    { label: 'Crons', href: '#/workspace/crons' },
    { label: esc(name) },
  ])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">${esc(name)}</h1>
    </div>
    <div class="empty-state">
      <h3>Coming soon</h3>
      <p>Cron detail is not yet implemented. See tasks.md → Task 013.</p>
    </div>
  `)
}
