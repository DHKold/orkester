// ── Workspace: WorkRunner detail page ─────────────────────────────────────
// Shows spec, status with live load gauge (active_work_runs / active_task_runs),
// state history, and the list of WorkRuns executed by this runner.
// TODO: Implement. See tasks.md → Task 008.

import { setApp, setBreadcrumb, esc } from '../../utils.js'

export function renderRunnerDetail({ name } = {}) {
  setBreadcrumb([
    { label: 'Workspace' },
    { label: 'Runners', href: '#/workspace/runners' },
    { label: esc(name) },
  ])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">${esc(name)}</h1>
    </div>
    <div class="empty-state">
      <h3>Coming soon</h3>
      <p>WorkRunner detail is not yet implemented. See tasks.md → Task 008.</p>
    </div>
  `)
}
