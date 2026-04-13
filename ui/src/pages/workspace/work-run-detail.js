// ── Workspace: Work Run detail page ───────────────────────────────────────
// Shows: header metadata, stats grid, DAG visualization, step cards with
// expandable TaskRun details (inputs/outputs/logs), structured run logs.
// Auto-refreshes every 3 s until terminal state.
// TODO: Implement. See tasks.md → Task 011.

import { setApp, setBreadcrumb, esc } from '../../utils.js'

export function renderWorkRunDetail({ name } = {}) {
  setBreadcrumb([
    { label: 'Workspace' },
    { label: 'Work Runs', href: '#/workspace/work-runs' },
    { label: esc(name) },
  ])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">${esc(name)}</h1>
    </div>
    <div class="empty-state">
      <h3>Coming soon</h3>
      <p>Work Run detail is not yet implemented. See tasks.md → Task 011.</p>
    </div>
  `)
}
