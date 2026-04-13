// ── Catalog list page ──────────────────────────────────────────────────────
// TODO: Implement generic document list with search and kind/name filters,
// scoped to the active namespace. See tasks.md → Task 005.

import { setApp, setBreadcrumb } from '../../utils.js'

export function renderCatalog({ query = {} } = {}) {
  setBreadcrumb([{ label: 'Catalog' }])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Catalog</h1>
    </div>
    <div class="empty-state">
      <h3>Coming soon</h3>
      <p>The Catalog list is not yet implemented. See tasks.md → Task 005.</p>
    </div>
  `)
}
