import { createDocument, updateDocument, searchDocuments }    from '../../api.js'
import { getActiveNamespace }                                  from '../../state.js'
import { setApp, setBreadcrumb, esc }                          from '../../utils.js'
import { navigate, setCleanup }                                from '../../router.js'
import { toastError, toastSuccess }                            from '../../components/toast.js'
import { EditorView, lineNumbers, highlightActiveLine }        from '@codemirror/view'
import { syntaxHighlighting, defaultHighlightStyle }           from '@codemirror/language'
import { json }                                                from '@codemirror/lang-json'

let specEditor       = null
let existingVersions = new Set()  // versions that already exist for this kind/ns/name
let originalVersion  = null       // version being edited (null = new doc)

// SemVer: MAJOR.MINOR or MAJOR.MINOR.PATCH with optional pre-release label
const SEMVER_RE = /[0-9]+\.[0-9]+(\.[0-9]+)?(-[a-zA-Z0-9][a-zA-Z0-9\-]*)?/

// ── Entry point ────────────────────────────────────────────────────────────

export async function renderCatalogNew({ query = {} } = {}) {
  const activeNs = getActiveNamespace()
  const isEdit   = query.edit === '1'

  const prefillNs = query.ns !== undefined
    ? (query.ns === 'global' ? null : (query.ns || null))
    : activeNs

  const prefillKind    = query.kind    ?? ''
  const prefillName    = query.name    ?? ''
  const prefillVersion = query.version ?? ''
  // baseVersion = the source doc's version to pre-fill from
  const baseVersion    = query.baseVersion ?? prefillVersion

  // Remember original for update-vs-create decision at submit time
  originalVersion  = isEdit ? prefillVersion : null
  existingVersions = new Set()

  setBreadcrumb([
    { label: 'Catalog', href: '#/catalog' },
    { label: isEdit ? 'Edit Document' : 'New Document' },
  ])
  setApp(`<div class="loading-state" aria-busy="true">Initializing…</div>`)

  // Fetch namespace list for the dropdown
  let namespaces = []
  try {
    const data = await searchDocuments({ when: { eq: { lhs: { field: 'kind' }, rhs: { value: 'workaholic/Namespace:1.0' } } } })
    namespaces = Array.isArray(data) ? data : []
  } catch (_) { /* fall back to empty list */ }

  // Load all existing versions for this kind/ns/name to enable pre-fill + conflict check
  let baseDoc = null
  if (prefillKind && prefillName) {
    try {
      const all = await searchDocuments({ all: [] })
      const forDoc = (Array.isArray(all) ? all : []).filter(
        d => d.kind === prefillKind
          && d.name === prefillName
          && (d.metadata?.namespace ?? null) === prefillNs
      )
      existingVersions = new Set(forDoc.map(d => d.version))
      if (baseVersion) {
        baseDoc = forDoc.find(d => d.version === baseVersion) ?? null
      }
    } catch (_) { /* no pre-fill, not fatal */ }
  }

  renderForm({ prefillNs, prefillKind, prefillName, prefillVersion, namespaces, baseDoc, isEdit })
}

// ── Form render ────────────────────────────────────────────────────────────

function renderForm({ prefillNs, prefillKind, prefillName, prefillVersion = '', namespaces, baseDoc, isEdit = false }) {
  const meta  = baseDoc?.metadata ?? {}
  const extra = { ...meta }
  delete extra.namespace; delete extra.owner; delete extra.description; delete extra.tags

  const tags = Array.isArray(meta.tags) ? meta.tags.join(', ') : (meta.tags ?? '')

  const nsOpts = [
    `<option value="" ${prefillNs === null ? 'selected' : ''}>— Global (no namespace) —</option>`,
    ...namespaces.map(ns => {
      const nsName = ns.name ?? ''
      const sel    = prefillNs === nsName ? 'selected' : ''
      return `<option value="${esc(nsName)}" ${sel}>${esc(nsName)}</option>`
    }),
  ].join('')

  const extraRows = Object.entries(extra)
    .map(([k, v]) => extraRowHtml(k, typeof v === 'string' ? v : JSON.stringify(v)))
    .join('')

  const docTitle  = isEdit && prefillName ? esc(prefillName) : 'New Document'
  const submitLbl = isEdit ? 'Save Changes' : 'Create Document'

  // Status section: read-only actual status when editing; info note when new
  const status    = baseDoc?.status
  const hasStatus = status && Object.keys(status).length > 0
  const statusSection = isEdit
    ? (hasStatus
        ? `<pre class="code-block" style="margin:0;max-height:160px;overflow:auto">${esc(JSON.stringify(status, null, 2))}</pre>`
        : `<p style="color:var(--text-3);font-size:0.875rem;margin:0.25rem 0">No status data available.</p>`)
    : `<div class="new-doc-status-note">
        <svg viewBox="0 0 16 16" width="15" height="15" fill="none" stroke="currentColor"
             stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"
             style="flex-shrink:0;margin-top:2px">
          <circle cx="8" cy="8" r="7"/>
          <path d="M8 5v.01"/>
          <path d="M8 8v4"/>
        </svg>
        The <code>status</code> field is managed at runtime — it will be
        <strong>empty</strong> on creation.
      </div>`

  const detailHref = prefillKind && prefillName
    ? `#/catalog/detail?${new URLSearchParams({ kind: prefillKind, ns: prefillNs ?? 'global', name: prefillName })}`
    : '#/catalog'

  setBreadcrumb([
    { label: 'Catalog', href: '#/catalog' },
    ...(prefillName ? [{ label: prefillName, href: detailHref }] : []),
    { label: isEdit ? 'Edit' : 'New Document' },
  ])

  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">${docTitle}</h1>
    </div>

    <form id="new-doc-form" style="max-width:720px">

      <!-- ── Identity ──────────────────────────────────────────────────── -->
      <div class="form-section-header">Identity</div>
      <div class="new-doc-grid">
        <label class="new-doc-label" for="f-kind">Kind <span class="req">*</span></label>
        <input id="f-kind" class="new-doc-input" type="text"
               placeholder="e.g. workaholic/Work:1.0" value="${esc(prefillKind)}" required />

        <label class="new-doc-label" for="f-name">Name <span class="req">*</span></label>
        <input id="f-name" class="new-doc-input" type="text"
               placeholder="e.g. my-workflow" value="${esc(prefillName)}" required />

        <label class="new-doc-label" for="f-version">Version <span class="req">*</span></label>
        <div>
          <input id="f-version" class="new-doc-input" type="text"
                 placeholder="e.g. 1.0.0" value="${esc(prefillVersion)}" required
                 pattern="${SEMVER_RE.source}"
                 title="SemVer required: MAJOR.MINOR or MAJOR.MINOR.PATCH" />
          <div class="new-doc-hint">SemVer format — e.g. <code>1.0</code> or <code>1.2.3</code></div>
        </div>
      </div>

      <!-- ── Metadata ───────────────────────────────────────────────────── -->
      <div class="form-section-header" style="margin-top:1.75rem">Metadata</div>
      <div class="new-doc-grid">
        <label class="new-doc-label" for="f-namespace">Namespace</label>
        <select id="f-namespace" class="new-doc-input">${nsOpts}</select>

        <label class="new-doc-label" for="f-owner">Owner</label>
        <input id="f-owner" class="new-doc-input" type="text"
               placeholder="Optional owner" value="${esc(meta.owner ?? '')}" />

        <label class="new-doc-label" for="f-description">Description</label>
        <input id="f-description" class="new-doc-input" type="text"
               placeholder="Optional description" value="${esc(meta.description ?? '')}" />

        <label class="new-doc-label" for="f-tags">Tags</label>
        <input id="f-tags" class="new-doc-input" type="text"
               placeholder="Comma-separated, e.g. prod, v2" value="${esc(tags)}" />
      </div>

      <!-- ── Extra metadata ─────────────────────────────────────────────── -->
      <div class="form-section-header" style="margin-top:1.75rem">Extra Metadata</div>
      <div id="extra-meta-rows">${extraRows}</div>
      <button type="button" class="btn btn-ghost btn-sm" id="add-extra-btn"
              style="margin-top:0.5rem">+ Add field</button>

      <!-- ── Status ─────────────────────────────────────────────────────── -->
      <div class="form-section-header" style="margin-top:1.75rem">Status</div>
      ${statusSection}

      <!-- ── Spec ───────────────────────────────────────────────────────── -->
      <div class="form-section-header" style="margin-top:1.75rem">
        Spec (JSON) <span class="req">*</span>
      </div>
      <div id="spec-editor-wrap" class="cm-wrap" style="min-height:180px"></div>

      <div style="margin-top:1.25rem;display:flex;gap:0.5rem">
        <button type="submit" class="btn btn-primary" id="submit-btn">${submitLbl}</button>
        <button type="button" class="btn btn-secondary" id="cancel-btn">Cancel</button>
      </div>
    </form>
  `)

  const initialSpec = baseDoc?.spec !== undefined
    ? JSON.stringify(baseDoc.spec, null, 2)
    : '{}'

  specEditor = new EditorView({
    doc:        initialSpec,
    extensions: [json(), lineNumbers(), highlightActiveLine(), syntaxHighlighting(defaultHighlightStyle)],
    parent:     document.getElementById('spec-editor-wrap'),
  })

  setCleanup(() => { specEditor?.destroy(); specEditor = null })

  document.getElementById('cancel-btn')?.addEventListener('click', () => navigate(detailHref))

  document.getElementById('add-extra-btn')?.addEventListener('click', addExtraRow)

  document.getElementById('extra-meta-rows')?.addEventListener('click', e => {
    const row = e.target.closest('.extra-meta-row')
    if (row && e.target.closest('.extra-row-remove')) row.remove()
  })

  document.getElementById('new-doc-form')?.addEventListener('submit', async e => {
    e.preventDefault()
    await handleSubmit()
  })
}

// ── Extra metadata row helpers ─────────────────────────────────────────────

function extraRowHtml(key = '', value = '') {
  return `
    <div class="extra-meta-row">
      <input class="new-doc-input extra-key"   type="text" placeholder="Key"   value="${esc(key)}" />
      <input class="new-doc-input extra-value" type="text" placeholder="Value" value="${esc(value)}" />
      <button type="button" class="btn btn-ghost btn-icon extra-row-remove" title="Remove">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
             width="14" height="14">
          <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>`
}

function addExtraRow() {
  document.getElementById('extra-meta-rows')
    ?.insertAdjacentHTML('beforeend', extraRowHtml())
}

// ── Submit handler ─────────────────────────────────────────────────────────

async function handleSubmit() {
  const kind    = document.getElementById('f-kind').value.trim()
  const name    = document.getElementById('f-name').value.trim()
  const version = document.getElementById('f-version').value.trim()
  const nsRaw   = document.getElementById('f-namespace').value.trim()
  const desc    = document.getElementById('f-description').value.trim()
  const owner   = document.getElementById('f-owner').value.trim()
  const tagsRaw = document.getElementById('f-tags').value.trim()
  const specRaw = specEditor?.state.doc.toString() ?? '{}'

  if (!kind || !name || !version) {
    toastError('Kind, Name, and Version are required.')
    return
  }

  if (!SEMVER_RE.test(version)) {
    toastError('Version must be SemVer: MAJOR.MINOR or MAJOR.MINOR.PATCH (e.g. 1.0.0)')
    return
  }

  let spec
  try {
    spec = JSON.parse(specRaw)
  } catch {
    toastError('Spec is not valid JSON — fix it before saving.')
    return
  }

  const extraFields = {}
  for (const row of document.querySelectorAll('.extra-meta-row')) {
    const k = row.querySelector('.extra-key')?.value.trim()
    const v = row.querySelector('.extra-value')?.value.trim()
    if (k) extraFields[k] = v ?? ''
  }

  const nsValue = nsRaw || null
  const tags    = tagsRaw ? tagsRaw.split(',').map(t => t.trim()).filter(Boolean) : undefined

  const resource = {
    kind, name, version,
    metadata: {
      ...(nsValue      ? { namespace:   nsValue } : {}),
      ...(desc         ? { description: desc    } : {}),
      ...(owner        ? { owner }                : {}),
      ...(tags?.length ? { tags }                 : {}),
      ...extraFields,
    },
    spec,
  }

  const docNs = nsValue || 'global'
  const id    = `${kind}/${docNs}/${name}/${version}`

  // Conflict check: warn when the target version exists but wasn't the one we opened for editing
  const versionExists = existingVersions.has(version)
  const isOverwrite   = versionExists && version !== originalVersion
  if (isOverwrite) {
    if (!confirm(`Version "${version}" already exists.\nSaving will overwrite that version. Continue?`)) return
  }

  const btn = document.getElementById('submit-btn')
  if (btn) btn.disabled = true

  try {
    // Use PUT when version already exists (edit in-place or confirmed overwrite), POST otherwise
    if (versionExists) {
      await updateDocument(id, resource)
    } else {
      await createDocument(id, resource)
    }
    toastSuccess(versionExists ? 'Document saved.' : 'Document created.')
    navigate(`#/catalog/detail?${new URLSearchParams({ kind, ns: docNs, name })}`)
  } catch (e) {
    toastError(`Failed to save: ${e.message}`)
    if (btn) btn.disabled = false
  }
}
