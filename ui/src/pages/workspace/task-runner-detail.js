// ── Workspace: TaskRunner detail page ─────────────────────────────────────
// Shows kind, state, state history, and metrics for a single TaskRunner.
// TaskRunners are read-only (managed by the system).
// TODO: Implement. See tasks.md → Task 009.

import { setApp, setBreadcrumb, esc } from '../../utils.js'

export function renderTaskRunnerDetail({ name } = {}) {
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
      <p>TaskRunner detail is not yet implemented. See tasks.md → Task 009.</p>
    </div>
  `)
}
