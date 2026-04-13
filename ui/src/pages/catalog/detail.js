// ── Catalog document detail page ───────────────────────────────────────────
// Navigated to via: #/catalog/detail?kind=workaholic%2FWork%3A1.0&name=my-work&ns=default
// TODO: Implement generic document viewer (spec + status as JSON + editable fields).
// See tasks.md → Task 006.

import { setApp, setBreadcrumb } from '../../utils.js'

export function renderCatalogDetail({ query = {} } = {}) {
  const { kind = '', name = '', ns = '' } = query
  setBreadcrumb([
    { label: 'Catalog', href: '#/catalog' },
    { label: name || kind || 'Document' },
  ])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">${name || 'Document'}</h1>
    </div>
    <div class="empty-state">
      <h3>Coming soon</h3>
      <p>The Catalog document detail view is not yet implemented. See tasks.md → Task 006.</p>
    </div>
  `)
}
