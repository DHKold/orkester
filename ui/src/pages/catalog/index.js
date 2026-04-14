// ── Catalog list page ──────────────────────────────────────────────────────

import { searchDocuments }                               from '../../api.js'
import { getActiveNamespace }                            from '../../state.js'
import { setApp, setBreadcrumb, esc, applySort, paginate } from '../../utils.js'
import { navigate, setCleanup }                          from '../../router.js'
import { renderTable, bindTable }                        from '../../components/table.js'
import { toastError }                                    from '../../components/toast.js'

const PAGE_SIZE = 25

// Module-level state — persists across navigations (preserves search/sort on back-nav)
const state = { q: '', kindFilter: '', nsFilter: '', sortKey: 'name', sortDir: 'asc', page: 1 }
let allGroups = []

// ── Entry point ────────────────────────────────────────────────────────────

export async function renderCatalog() {
  if (!state.nsFilter) state.nsFilter = getActiveNamespace() ?? ''

  setBreadcrumb([{ label: 'Catalog' }])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Catalog</h1>
    </div>
    <div class="toolbar" id="catalog-toolbar">
      <input  id="catalog-search"      class="toolbar-search" type="search"
              placeholder="Search kind, name, description…" value="${esc(state.q)}" />
      <select id="catalog-kind-filter" style="max-width:240px"></select>
      <select id="catalog-ns-filter"   style="max-width:180px"></select>
      <button class="btn btn-secondary btn-sm" id="catalog-refresh-btn">↺ Refresh</button>
    </div>
    <div class="table-wrap" id="catalog-table-wrap">
      <div class="loading-state" aria-busy="true">Loading…</div>
    </div>
  `)

  window.addEventListener('orkester:namespace-changed', onNsChange)
  setCleanup(() => window.removeEventListener('orkester:namespace-changed', onNsChange))

  await loadAndRender()
  bindToolbar()
}

function onNsChange() {
  state.nsFilter = getActiveNamespace() ?? ''
  state.page     = 1
  populateFilters()
  renderRows()
}

// ── Data loading ───────────────────────────────────────────────────────────

async function loadAndRender() {
  try {
    const results = await searchDocuments({ all: [] })
    allGroups = buildGroups(Array.isArray(results) ? results : [])
  } catch (e) {
    toastError(`Failed to load catalog: ${e.message}`)
    allGroups = []
  }
  populateFilters()
  renderRows()
}

// ── Grouping: one row per (kind, namespace, name) ──────────────────────────

function buildGroups(docs) {
  const map = new Map()
  for (const doc of docs) {
    const ns  = doc.metadata?.namespace || 'global'
    const key = `${doc.kind}\0${ns}\0${doc.name}`
    if (!map.has(key)) map.set(key, { kind: doc.kind, ns, name: doc.name, versions: [] })
    map.get(key).versions.push(doc)
  }
  for (const g of map.values()) {
    g.versions.sort((a, b) => versionCmp(b.version, a.version))
    g.latest = g.versions[0]
  }
  return Array.from(map.values())
}

function versionCmp(a = '', b = '') {
  const pa = a.split('.').map(n => parseInt(n, 10) || 0)
  const pb = b.split('.').map(n => parseInt(n, 10) || 0)
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0)
    if (d !== 0) return d
  }
  return 0
}

// ── Filter selects ─────────────────────────────────────────────────────────

function populateFilters() {
  const kindSel = document.getElementById('catalog-kind-filter')
  const nsSel   = document.getElementById('catalog-ns-filter')
  if (!kindSel) return

  const kinds = [...new Set(allGroups.map(g => g.kind))].sort()
  const nss   = [...new Set(allGroups.map(g => g.ns))].sort()

  kindSel.innerHTML =
    `<option value="">All Kinds</option>` +
    kinds.map(k => `<option value="${esc(k)}" ${state.kindFilter === k ? 'selected' : ''}>${esc(k)}</option>`).join('')

  nsSel.innerHTML =
    `<option value="">All Namespaces</option>` +
    nss.map(n => `<option value="${esc(n)}" ${state.nsFilter === n ? 'selected' : ''}>${esc(n)}</option>`).join('')
}

function getFiltered() {
  let r = allGroups
  if (state.kindFilter) r = r.filter(g => g.kind === state.kindFilter)
  if (state.nsFilter)   r = r.filter(g => g.ns   === state.nsFilter)
  if (state.q) {
    const q = state.q.toLowerCase()
    r = r.filter(g =>
      g.kind.toLowerCase().includes(q) ||
      g.name.toLowerCase().includes(q) ||
      (g.latest?.metadata?.description ?? '').toLowerCase().includes(q) ||
      (g.latest?.metadata?.owner       ?? '').toLowerCase().includes(q)
    )
  }
  return r
}

// ── Table ──────────────────────────────────────────────────────────────────

const COLS = [
  {
    key: 'kind', label: 'Kind', sortable: true,
    render: (row) => `<code class="kind-chip">${esc(row.kind)}</code>`,
  },
  {
    key: 'ns', label: 'Namespace', sortable: true,
    render: (row) => row.ns === 'global'
      ? `<span style="color:var(--text-3)">—</span>`
      : esc(row.ns),
  },
  {
    key: 'name', label: 'Name', sortable: true,
    render: (row) => `<strong>${esc(row.name)}</strong>`,
  },
  {
    key: '_v', label: 'Versions', sortable: false,
    render: (row) => {
      const n = row.versions.length
      const v = esc(row.versions[0]?.version ?? '?')
      return n > 1
        ? `<span class="version-bubble" title="${n} versions">${n}</span> <span class="version-label">${v}</span>`
        : `<span class="version-label">${v}</span>`
    },
  },
  {
    key: 'latest.metadata.description', label: 'Description', sortable: false,
    render: (row) => `<span class="cell-muted">${esc(row.latest?.metadata?.description ?? '')}</span>`,
  },
  {
    key: 'latest.metadata.owner', label: 'Owner', sortable: true,
    render: (row) => `<span class="cell-muted">${esc(row.latest?.metadata?.owner ?? '—')}</span>`,
  },
]

function renderRows() {
  const filtered = getFiltered()
  const sorted   = applySort(filtered, state.sortKey, state.sortDir)
  const paged    = paginate(sorted, state.page, PAGE_SIZE)
  const wrap     = document.getElementById('catalog-table-wrap')
  if (!wrap) return

  wrap.innerHTML = renderTable({
    columns:  COLS,
    rows:     paged,
    total:    sorted.length,
    page:     state.page,
    pageSize: PAGE_SIZE,
    sortKey:  state.sortKey,
    sortDir:  state.sortDir,
    emptyMsg: 'No documents match the current filters.',
  })

  bindTable('catalog-table-wrap', {
    rows:       paged,
    sortKey:    state.sortKey,
    sortDir:    state.sortDir,
    onSort:     (key, dir) => { state.sortKey = key; state.sortDir = dir; state.page = 1; renderRows() },
    onPage:     (p)        => { state.page = p; renderRows() },
    onRowClick: (row)      => {
      navigate(`#/catalog/detail?${new URLSearchParams({ kind: row.kind, ns: row.ns, name: row.name })}`)
    },
  })
}

// ── Toolbar ────────────────────────────────────────────────────────────────

function bindToolbar() {
  document.getElementById('catalog-search')?.addEventListener('input', e => {
    state.q = e.target.value; state.page = 1; renderRows()
  })
  document.getElementById('catalog-kind-filter')?.addEventListener('change', e => {
    state.kindFilter = e.target.value; state.page = 1; renderRows()
  })
  document.getElementById('catalog-ns-filter')?.addEventListener('change', e => {
    state.nsFilter = e.target.value; state.page = 1; renderRows()
  })
  document.getElementById('catalog-refresh-btn')?.addEventListener('click', loadAndRender)
}
