// ── Catalog list page ──────────────────────────────────────────────────────

import { searchDocuments, deleteDocument }                  from '../../api.js'
import { getActiveNamespace }                               from '../../state.js'
import { setApp, setBreadcrumb, esc, applySort, paginate }  from '../../utils.js'
import { navigate, setCleanup }                             from '../../router.js'
import { renderTable, bindTable }                           from '../../components/table.js'
import { toastError, toastSuccess }                         from '../../components/toast.js'

const PAGE_SIZE = 25

// Module-level state — persists across navigations
const state    = { q: '', kindFilter: '', sortKey: 'name', sortDir: 'asc', page: 1 }
const selected = new Set()  // rowKeys of selected groups
let   allGroups = []

// ── Entry point ────────────────────────────────────────────────────────────

export async function renderCatalog() {
  setBreadcrumb([{ label: 'Catalog' }])
  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Catalog</h1>
      <div class="page-title-actions">
        <button class="btn btn-primary btn-sm" id="catalog-new-btn">+ New Document</button>
      </div>
    </div>
    <div class="toolbar" id="catalog-toolbar">
      <input  id="catalog-search"      class="toolbar-search" type="search"
              placeholder="Search kind, name, description…" value="${esc(state.q)}" />
      <select id="catalog-kind-filter" style="max-width:240px"></select>
      <button class="btn btn-secondary btn-sm" id="catalog-refresh-btn">↺ Refresh</button>
    </div>
    <div id="bulk-bar" class="bulk-bar hidden">
      <span id="bulk-count" class="bulk-count"></span>
      <button class="btn btn-danger btn-sm" id="bulk-delete-btn">Delete selected</button>
      <button class="btn btn-ghost  btn-sm" id="bulk-clear-btn">Clear</button>
    </div>
    <div class="table-wrap" id="catalog-table-wrap">
      <div class="loading-state" aria-busy="true">Loading…</div>
    </div>
  `)

  window.addEventListener('orkester:namespace-changed', onNsChange)
  setCleanup(() => window.removeEventListener('orkester:namespace-changed', onNsChange))

  document.getElementById('catalog-new-btn')?.addEventListener('click', () => {
    const ns = getActiveNamespace() ?? ''
    navigate(`#/catalog/new${ns ? '?ns=' + encodeURIComponent(ns) : ''}`)
  })

  await loadAndRender()
  bindToolbar()
}

function onNsChange() {
  state.page = 1
  selected.clear()
  populateKindFilter()
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
  selected.clear()
  populateKindFilter()
  renderRows()
}

// ── Grouping: one row per (kind, namespace, name) ──────────────────────────

function rowKey(row) { return `${row.kind}\0${row.ns}\0${row.name}` }

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

// ── Kind filter ────────────────────────────────────────────────────────────

function populateKindFilter() {
  const sel = document.getElementById('catalog-kind-filter')
  if (!sel) return
  const kinds = [...new Set(allGroups.map(g => g.kind))].sort()
  sel.innerHTML =
    `<option value="">All Kinds</option>` +
    kinds.map(k => `<option value="${esc(k)}" ${state.kindFilter === k ? 'selected' : ''}>${esc(k)}</option>`).join('')
}

function getFiltered() {
  const ns = getActiveNamespace()   // null → no filter (global mode)
  let r = allGroups
  if (ns) r = r.filter(g => g.ns === ns)
  if (state.kindFilter) r = r.filter(g => g.kind === state.kindFilter)
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
    key: '_sel',
    labelHtml: '<input type="checkbox" id="select-all-check" title="Select all on this page">',
    sortable: false, width: '2.5rem',
    render: (row) => {
      const k = rowKey(row)
      return `<input type="checkbox" class="row-check" data-key="${esc(k)}" ${selected.has(k) ? 'checked' : ''}>`
    },
  },
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

  bindCheckboxes(paged)
  renderBulkBar()
}

// ── Bulk selection ─────────────────────────────────────────────────────────

function bindCheckboxes(paged) {
  const selectAll = document.getElementById('select-all-check')
  if (selectAll) {
    selectAll.addEventListener('change', () => {
      if (selectAll.checked) paged.forEach(r => selected.add(rowKey(r)))
      else                   paged.forEach(r => selected.delete(rowKey(r)))
      renderRows()   // re-render to reflect checkbox states + update bulk bar
    })
  }

  document.querySelectorAll('#catalog-table-wrap .row-check').forEach(cb => {
    cb.addEventListener('change', () => {
      if (cb.checked) selected.add(cb.dataset.key)
      else            selected.delete(cb.dataset.key)
      renderBulkBar()
    })
  })
}

function renderBulkBar() {
  const bar = document.getElementById('bulk-bar')
  if (!bar) return
  const n = selected.size
  bar.classList.toggle('hidden', n === 0)
  const el = document.getElementById('bulk-count')
  if (el) el.textContent = `${n} document group${n !== 1 ? 's' : ''} selected`
}

async function bulkDelete() {
  const keys = [...selected]
  if (keys.length === 0) return
  const count = keys.length
  if (!confirm(`Delete ${count} document group${count !== 1 ? 's' : ''}?\nThis deletes ALL versions. This cannot be undone.`)) return

  const toDelete = keys.flatMap(k => {
    const [kind, ns, name] = k.split('\0')
    return allGroups.find(g => g.kind === kind && g.ns === ns && g.name === name)?.versions ?? []
  })

  let errors = 0
  for (const doc of toDelete) {
    try {
      const docNs = doc.metadata?.namespace || 'global'
      await deleteDocument(`${doc.kind}/${docNs}/${doc.name}/${doc.version}`)
    } catch { errors++ }
  }

  if (errors) toastError(`${errors} deletion(s) failed.`)
  else        toastSuccess(`Deleted ${count} document group${count !== 1 ? 's' : ''}.`)

  selected.clear()
  await loadAndRender()
}

// ── Toolbar ────────────────────────────────────────────────────────────────

function bindToolbar() {
  document.getElementById('catalog-search')?.addEventListener('input', e => {
    state.q = e.target.value; state.page = 1; renderRows()
  })
  document.getElementById('catalog-kind-filter')?.addEventListener('change', e => {
    state.kindFilter = e.target.value; state.page = 1; renderRows()
  })
  document.getElementById('catalog-refresh-btn')?.addEventListener('click', loadAndRender)
  document.getElementById('bulk-delete-btn')?.addEventListener('click', bulkDelete)
  document.getElementById('bulk-clear-btn')?.addEventListener('click', () => {
    selected.clear(); renderRows()
  })
}
