// ── Settings page ──────────────────────────────────────────────────────────
// Displays and edits settings documents stored in the Catalog as
// core/UserSettings and core/AppConfig documents.
// TODO: Implement. See tasks.md → Task 015.

import { setApp, setBreadcrumb } from '../utils.js'

export function renderSettings({ query = {} } = {}) {
  setBreadcrumb([{ label: 'Settings' }])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Settings</h1>
    </div>
    <div class="empty-state">
      <h3>Coming soon</h3>
      <p>The Settings page is not yet implemented. See tasks.md → Task 015.</p>
    </div>
  `)
}
