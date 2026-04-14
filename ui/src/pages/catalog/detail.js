// ── Catalog document detail page ───────────────────────────────────────────
// Route: #/catalog/detail?kind=...&ns=...&name=...  (version optional)
// Shows all versions of a (kind, ns, name) document grouped together.
// Spec is editable inline via CodeMirror 6; Status and Raw are read-only.

import { searchDocuments, updateDocument, deleteDocument } from '../../api.js'
import { setApp, setBreadcrumb, esc, fmtDate }             from '../../utils.js'
import { navigate, setCleanup }                            from '../../router.js'
import { toastSuccess, toastError }                        from '../../components/toast.js'
import { EditorView, lineNumbers, highlightActiveLine }  from '@codemirror/view'
import { syntaxHighlighting, defaultHighlightStyle }      from '@codemirror/language'
import { json }                                           from '@codemirror/lang-json'

// ── Page-level state (reset on each navigation) ────────────────────────────
let docs          = []   // all version docs for current (kind, ns, name), latest-first
let activeVersion = ''
let activeTab     = 'spec'
let editMode      = false
let specEditor    = null  // CodeMirror for spec editing
let rawEditor     = null  // CodeMirror for raw read-only view

// ── Entry point ────────────────────────────────────────────────────────────

export async function renderCatalogDetail({ query = {} } = {}) {
  const { kind = '', ns = 'global', name = '' } = query

  // Reset state from previous navigation
  destroyEditors()
  docs          = []
  activeTab     = 'spec'
  editMode      = false

  setBreadcrumb([
    { label: 'Catalog', href: '#/catalog' },
    { label: kind, href: `#/catalog?${new URLSearchParams({ kind })}` },
    { label: name },
  ])
  setApp(`<div class="loading-state" aria-busy="true">Loading…</div>`)
  setCleanup(destroyEditors)

  try {
    // Fetch all versions of this document (filter client-side)
    const results = await searchDocuments({all:[]})
    const targetNs = ns === 'global' ? null : ns
    docs = (results ?? [])
      .filter(d =>
        d.kind === kind &&
        d.name === name &&
        (d.metadata?.namespace ?? null) === targetNs
      )
      .sort((a, b) => versionCmp(b.version, a.version))

    if (docs.length === 0) {
      setApp(`
        <div class="empty-state">
          <h3>Not found</h3>
          <p>No document found for <code class="kind-chip">${esc(kind)}</code> <strong>${esc(name)}</strong>
          ${ns !== 'global' ? ` in namespace <em>${esc(ns)}</em>` : '(global)'}.
          </p>
          <a href="#/catalog" class="btn btn-secondary btn-sm">← Back to Catalog</a>
        </div>`)
      return
    }

    // Honour explicit version from query, otherwise use latest
    activeVersion = (query.version && docs.find(d => d.version === query.version))
      ? query.version
      : docs[0].version

    renderPage(kind, ns, name)
  } catch (e) {
    toastError(`Failed to load document: ${e.message}`)
    setApp(`<div class="empty-state"><h3>Error</h3><p>${esc(e.message)}</p></div>`)
  }
}

// ── Helpers ────────────────────────────────────────────────────────────────

function versionCmp(a = '', b = '') {
  const pa = a.split('.').map(n => parseInt(n, 10) || 0)
  const pb = b.split('.').map(n => parseInt(n, 10) || 0)
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0)
    if (d !== 0) return d
  }
  return 0
}

function activeDoc() {
  return docs.find(d => d.version === activeVersion) ?? docs[0]
}

function docId(doc) {
  return `${doc.kind}/${doc.metadata?.namespace || 'global'}/${doc.name}/${doc.version}`
}

// ── Full page render ───────────────────────────────────────────────────────

function renderPage(kind, ns, name) {
  destroyEditors()
  const doc  = activeDoc()
  const meta = doc.metadata ?? {}
  const tags = (meta.tags ?? []).join(', ')

  setApp(`
    <div class="page-title-row">
      <div style="display:flex;align-items:baseline;gap:0.75rem;flex-wrap:wrap;min-width:0">
        <h1 class="page-title">${esc(name)}</h1>
        <code class="kind-chip">${esc(kind)}</code>
      </div>
      <div class="page-title-actions">
        <button class="btn btn-secondary btn-sm" id="btn-new-version">+ New version</button>
        <button class="btn btn-danger btn-sm"    id="btn-delete-version">Delete version</button>
      </div>
    </div>

    <!-- Metadata -->
    <div class="detail-grid" style="margin-bottom:1.25rem">
      <div class="detail-field">
        <label>Namespace</label>
        <div class="value">${ns === 'global'
          ? '<span style="color:var(--text-3)">global</span>'
          : esc(ns)}</div>
      </div>
      <div class="detail-field">
        <label>Owner</label>
        <div class="value">${esc(meta.owner || '—')}</div>
      </div>
      <div class="detail-field" style="grid-column:span 2">
        <label>Description</label>
        <div class="value">${esc(meta.description || '—')}</div>
      </div>
      <div class="detail-field">
        <label>Tags</label>
        <div class="value">${tags
          ? tags.split(',').map(t => `<span class="version-bubble" style="border-radius:var(--radius-sm)">${esc(t.trim())}</span>`).join(' ')
          : '<span style="color:var(--text-3)">—</span>'}</div>
      </div>
    </div>

    <!-- Version selector -->
    <div class="version-selector" id="version-selector">
      ${renderVersionSelector()}
    </div>

    <!-- Tab bar -->
    <div class="tabs" style="margin-top:1.25rem" id="detail-tabs">
      <button class="tab-btn${activeTab === 'spec'   ? ' active' : ''}" data-tab="spec">Spec</button>
      <button class="tab-btn${activeTab === 'status' ? ' active' : ''}" data-tab="status">Status</button>
      <button class="tab-btn${activeTab === 'raw'    ? ' active' : ''}" data-tab="raw">Raw</button>
    </div>

    <!-- Tab panels -->
    <div id="tab-spec">${renderSpecPanel(doc)}</div>
    <div id="tab-status" style="display:none">${renderStatusPanel(doc)}</div>
    <div id="tab-raw"    style="display:none">
      <div id="raw-editor-wrap" class="cm-wrap"></div>
    </div>
  `)

  bindPageEvents(kind, ns, name)
}

// ── Version selector ───────────────────────────────────────────────────────

function renderVersionSelector() {
  if (docs.length === 1) {
    return `<span class="version-single">v${esc(docs[0].version)}</span>`
  }
  return `
    <label for="version-select" class="version-select-label">Version</label>
    <select id="version-select" class="version-select">
      ${docs.map((d, i) => `
        <option value="${esc(d.version)}" ${d.version === activeVersion ? 'selected' : ''}>
          v${esc(d.version)}${i === 0 ? ' (latest)' : ''}
        </option>`).join('')}
    </select>`
}

// ── Tab panels ─────────────────────────────────────────────────────────────

function renderSpecPanel(doc) {
  if (editMode) {
    return `
      <div class="spec-edit-toolbar">
        <button class="btn btn-primary   btn-sm" id="btn-spec-save">Save</button>
        <button class="btn btn-secondary btn-sm" id="btn-spec-cancel">Cancel</button>
      </div>
      <div id="spec-editor-wrap" class="cm-wrap"></div>`
  }
  return `
    <div style="display:flex;justify-content:flex-end;margin-bottom:0.5rem">
      <button class="btn btn-secondary btn-sm" id="btn-spec-edit">Edit</button>
    </div>
    <pre class="code-block">${esc(JSON.stringify(doc.spec ?? {}, null, 2))}</pre>`
}

function renderStatusPanel(doc) {
  const status = doc.status
  if (!status || Object.keys(status).length === 0) {
    return `<p style="color:var(--text-3);padding:0.5rem 0">No status available.</p>`
  }
  return `<pre class="code-block">${esc(JSON.stringify(status, null, 2))}</pre>`
}

// ── CodeMirror helpers ─────────────────────────────────────────────────────

function mountSpecEditor(doc) {
  destroyEditor('spec')
  const wrap = document.getElementById('spec-editor-wrap')
  if (!wrap) return
  specEditor = new EditorView({
    doc:        JSON.stringify(doc.spec ?? {}, null, 2),
    extensions: [json(), lineNumbers(), highlightActiveLine(), syntaxHighlighting(defaultHighlightStyle)],
    parent:     wrap,
  })
}

function mountRawEditor(doc) {
  destroyEditor('raw')
  const wrap = document.getElementById('raw-editor-wrap')
  if (!wrap) return
  rawEditor = new EditorView({
    doc:        JSON.stringify(doc, null, 2),
    extensions: [json(), lineNumbers(), syntaxHighlighting(defaultHighlightStyle), EditorView.editable.of(false)],
    parent:     wrap,
  })
}

function destroyEditor(which) {
  if (which === 'spec' && specEditor) { specEditor.destroy(); specEditor = null }
  if (which === 'raw'  && rawEditor)  { rawEditor.destroy();  rawEditor  = null }
}

function destroyEditors() {
  destroyEditor('spec')
  destroyEditor('raw')
}

// ── Event wiring ───────────────────────────────────────────────────────────

function bindPageEvents(kind, ns, name) {
  // Version selector
  document.getElementById('version-select')?.addEventListener('change', e => {
    activeVersion = e.target.value
    editMode      = false
    destroyEditors()
    const doc = activeDoc()
    document.getElementById('tab-spec').innerHTML   = renderSpecPanel(doc)
    document.getElementById('tab-status').innerHTML = renderStatusPanel(doc)
    if (activeTab === 'raw') { mountRawEditor(doc) }
    bindTabContent(kind, ns, name)
  })

  // Tab switching
  document.getElementById('detail-tabs')?.addEventListener('click', e => {
    const btn = e.target.closest('.tab-btn')
    if (!btn) return
    activeTab = btn.dataset.tab
    document.querySelectorAll('#detail-tabs .tab-btn').forEach(b =>
      b.classList.toggle('active', b.dataset.tab === activeTab)
    )
    document.getElementById('tab-spec').style.display   = activeTab === 'spec'   ? '' : 'none'
    document.getElementById('tab-status').style.display = activeTab === 'status' ? '' : 'none'
    document.getElementById('tab-raw').style.display    = activeTab === 'raw'    ? '' : 'none'
    if (activeTab === 'raw' && !rawEditor) mountRawEditor(activeDoc())
  })

  // New version (stubbed — Task 006b)
  document.getElementById('btn-new-version')?.addEventListener('click', () => {
    navigate(`#/catalog/new?${new URLSearchParams({ kind, ns, name })}`)
  })

  // Delete version
  document.getElementById('btn-delete-version')?.addEventListener('click', async () => {
    const doc = activeDoc()
    if (!confirm(`Delete version ${doc.version} of "${name}"?\nThis cannot be undone.`)) return
    try {
      await deleteDocument(docId(doc))
      toastSuccess(`Version ${doc.version} deleted.`)
      docs = docs.filter(d => d.version !== doc.version)
      if (docs.length > 0) {
        activeVersion = docs[0].version
        editMode      = false
        destroyEditors()
        renderPage(kind, ns, name)
      } else {
        navigate('#/catalog')
      }
    } catch (e) {
      toastError(`Delete failed: ${e.message}`)
    }
  })

  bindTabContent(kind, ns, name)
}

function bindTabContent(kind, ns, name) {
  // Enter edit mode
  document.getElementById('btn-spec-edit')?.addEventListener('click', () => {
    editMode = true
    document.getElementById('tab-spec').innerHTML = renderSpecPanel(activeDoc())
    mountSpecEditor(activeDoc())
    bindEditButtons(kind, ns, name)
  })

  bindEditButtons(kind, ns, name)
}

function bindEditButtons(kind, ns, name) {
  // Save
  document.getElementById('btn-spec-save')?.addEventListener('click', async () => {
    if (!specEditor) return
    let newSpec
    try {
      newSpec = JSON.parse(specEditor.state.doc.toString())
    } catch {
      toastError('Invalid JSON — fix syntax before saving.')
      return
    }
    const doc     = activeDoc()
    const updated = { ...doc, spec: newSpec }
    try {
      const saved = await updateDocument(docId(doc), updated)
      const idx   = docs.findIndex(d => d.version === activeVersion)
      if (idx >= 0) docs[idx] = saved ?? updated
      editMode = false
      destroyEditor('spec')
      document.getElementById('tab-spec').innerHTML = renderSpecPanel(activeDoc())
      bindTabContent(kind, ns, name)
      toastSuccess('Saved.')
    } catch (e) {
      toastError(`Save failed: ${e.message}`)
    }
  })

  // Cancel
  document.getElementById('btn-spec-cancel')?.addEventListener('click', () => {
    editMode = false
    destroyEditor('spec')
    document.getElementById('tab-spec').innerHTML = renderSpecPanel(activeDoc())
    bindTabContent(kind, ns, name)
  })
}
