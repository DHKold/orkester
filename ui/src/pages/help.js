// ── Help page ──────────────────────────────────────────────────────────────
// Displays help topics stored in the Catalog as core/HelpTopic documents.
// TODO: Implement. See tasks.md → Task 016.

import { setApp, setBreadcrumb } from '../utils.js'

export function renderHelp({ query = {} } = {}) {
  setBreadcrumb([{ label: 'Help' }])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Help</h1>
    </div>
    <div class="empty-state">
      <h3>Coming soon</h3>
      <p>The Help page is not yet implemented. See tasks.md → Task 016.</p>
    </div>
  `)
}
