// ── Settings page ──────────────────────────────────────────────────────────
// Sub-nav tabs:
//   ?tab=user  → core/UserSettings:1.0  (per-user, named <userId>-settings)
//   ?tab=app   → core/AppSettings:1.0   (global app config, named app-settings)

import { searchDocuments, createDocument, updateDocument }  from '../api.js'
import { getActiveUser }                                     from '../state.js'
import { setApp, setBreadcrumb, esc }                        from '../utils.js'
import { navigate, setCleanup }                              from '../router.js'
import { toastError, toastSuccess }                          from '../components/toast.js'

const USER_KIND = 'core/UserSettings:1.0'
const APP_KIND  = 'core/AppSettings:1.0'
const VERSION   = '1.0.0'

// Module-level cache — reset when navigating away
let loaded  = false
let userDoc = null   // null = not found in catalog
let appDoc  = null

// ── Entry point ────────────────────────────────────────────────────────────

export async function renderSettings({ query = {} } = {}) {
  const tab = query.tab ?? 'user'
  setBreadcrumb([{ label: 'Settings' }])

  if (!loaded) {
    setApp(`<div class="loading-state" aria-busy="true">Loading settings…</div>`)
    await loadDocs()
    setCleanup(() => { loaded = false; userDoc = null; appDoc = null })
  }

  renderPage(tab)
}

// ── Data loading ───────────────────────────────────────────────────────────

async function loadDocs() {
  const userId   = getActiveUser().id
  const userName = docNameForUser(userId)
  try {
    const all  = await searchDocuments({ all: [] })
    const docs = Array.isArray(all) ? all : []
    userDoc = docs.find(d => d.kind === USER_KIND && d.name === userName) ?? null
    appDoc  = docs.find(d => d.kind === APP_KIND  && d.name === 'app-settings') ?? null
  } catch (e) {
    toastError(`Failed to load settings: ${e.message}`)
  }
  loaded = true
}

// ── Page render ────────────────────────────────────────────────────────────

function renderPage(tab) {
  const isUser = tab !== 'app'
  const doc    = isUser ? userDoc : appDoc
  const kind   = isUser ? USER_KIND : APP_KIND
  const label  = isUser ? 'User Settings' : 'Application Settings'

  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Settings</h1>
    </div>

    <nav class="sub-nav">
      <a href="#/settings?tab=user" class="${isUser  ? 'active' : ''}">User Settings</a>
      <a href="#/settings?tab=app"  class="${!isUser ? 'active' : ''}">Application Settings</a>
    </nav>

    <div style="max-width:640px;margin-top:1.25rem">
      ${renderForm(doc, label)}
      <div style="margin-top:1.25rem;display:flex;gap:0.5rem">
        <button class="btn btn-primary" id="settings-save-btn">Save ${label}</button>
        <button class="btn btn-secondary" id="settings-reset-btn">Reset</button>
      </div>
    </div>
  `)

  // Intercept sub-nav clicks — switch tabs without full re-fetch
  document.querySelectorAll('.sub-nav a').forEach(a => {
    a.addEventListener('click', e => {
      e.preventDefault()
      const newTab = new URLSearchParams(a.getAttribute('href').split('?')[1] ?? '').get('tab') ?? 'user'
      window.location.hash = a.getAttribute('href')
      // The router will call renderSettings again; because loaded=true it won't re-fetch
    })
  })

  document.getElementById('settings-save-btn')?.addEventListener('click', () => handleSave(tab))
  document.getElementById('settings-reset-btn')?.addEventListener('click', () => renderPage(tab))
}

// ── Form rendering ─────────────────────────────────────────────────────────

function renderForm(doc, sectionLabel) {
  const props = doc?.spec?.properties ?? []

  if (props.length === 0) {
    return `
      <div class="empty-state" style="padding:2rem 0;text-align:left;border:1px dashed var(--border-2);border-radius:var(--radius-lg);padding:1.5rem">
        <p style="color:var(--text-3);margin:0">No settings defined. Add properties to the
        <code>${doc ? esc(doc.kind) : sectionLabel}</code> catalog document to display them here.</p>
      </div>`
  }

  const rows = props.map((p, i) => {
    const label = esc(p.label ?? p.key)
    const desc  = p.description ? `<div class="settings-desc">${esc(p.description)}</div>` : ''
    const input = renderInput(p, i)
    return `
      <div class="settings-row">
        <div class="settings-label-cell">
          <label class="settings-label" for="sp-${i}">${label}</label>
          ${desc}
        </div>
        <div class="settings-input-cell">${input}</div>
      </div>`
  }).join('')

  return `<div class="settings-form">${rows}</div>`
}

function renderInput(prop, idx) {
  const id  = `sp-${idx}`
  const cls = 'new-doc-input settings-input'

  switch (prop.type) {
    case 'boolean':
      return `
        <label class="settings-toggle">
          <input type="checkbox" id="${id}" class="settings-checkbox"
                 data-key="${esc(prop.key)}" data-type="boolean"
                 ${prop.value ? 'checked' : ''} />
          <span class="settings-toggle-track"><span class="settings-toggle-thumb"></span></span>
        </label>`

    case 'number':
      return `<input type="number" id="${id}" class="${cls}"
                     data-key="${esc(prop.key)}" data-type="number"
                     value="${esc(String(prop.value ?? ''))}" />`

    case 'select':
      {
        const opts = (prop.options ?? []).map(o =>
          `<option value="${esc(o)}" ${o === prop.value ? 'selected' : ''}>${esc(o)}</option>`
        ).join('')
        return `<select id="${id}" class="${cls}"
                        data-key="${esc(prop.key)}" data-type="select">${opts}</select>`
      }

    default: // string and unknown
      return `<input type="text" id="${id}" class="${cls}"
                     data-key="${esc(prop.key)}" data-type="${esc(prop.type ?? 'string')}"
                     value="${esc(String(prop.value ?? ''))}" />`
  }
}

// ── Save handler ───────────────────────────────────────────────────────────

async function handleSave(tab) {
  const isUser   = tab !== 'app'
  const userId   = getActiveUser().id
  const userName = docNameForUser(userId)
  const docName  = isUser ? userName : 'app-settings'
  const kind     = isUser ? USER_KIND : APP_KIND
  const existing = isUser ? userDoc   : appDoc

  // Collect current form values
  const updatedProps = []
  document.querySelectorAll('[data-key]').forEach(el => {
    const key   = el.dataset.key
    const type  = el.dataset.type ?? 'string'
    let   value

    if (type === 'boolean') {
      value = el.checked
    } else if (type === 'number') {
      value = el.value === '' ? null : Number(el.value)
    } else {
      value = el.value
    }

    // Preserve other fields from original property (label, description, options, …)
    const original = (existing?.spec?.properties ?? []).find(p => p.key === key) ?? {}
    updatedProps.push({ ...original, key, value, type })
  })

  const resource = {
    kind,
    name:     docName,
    version:  VERSION,
    metadata: { ...(existing?.metadata ?? {}) },
    spec:     { properties: updatedProps },
  }

  const id  = `${kind}/global/${docName}/${VERSION}`
  const btn = document.getElementById('settings-save-btn')
  if (btn) btn.disabled = true

  try {
    if (existing) {
      await updateDocument(id, resource)
      if (isUser) userDoc = { ...existing, spec: resource.spec }
      else        appDoc  = { ...existing, spec: resource.spec }
    } else {
      await createDocument(id, resource)
      if (isUser) userDoc = resource
      else        appDoc  = resource
    }
    toastSuccess('Settings saved.')
  } catch (e) {
    toastError(`Failed to save: ${e.message}`)
  } finally {
    if (btn) btn.disabled = false
  }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/** Convert a user ID like "user:john" to a safe document name "user-john-settings". */
function docNameForUser(userId) {
  return userId.replace(/[^a-zA-Z0-9_-]/g, '-') + '-settings'
}

