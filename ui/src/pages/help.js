// ── Help page ──────────────────────────────────────────────────────────────
// List route:   #/help              → lists all listable HelpTopic docs
// Detail route: #/help?topic=<name> → renders a single topic as Markdown

import { searchDocuments }                    from '../api.js'
import { getActiveNamespace }                 from '../state.js'
import { setApp, setBreadcrumb, esc }          from '../utils.js'
import { navigate, setCleanup }               from '../router.js'
import { toastError }                         from '../components/toast.js'

// marked is loaded via CDN script tag in index.html as window.marked
const md = (src) => window.marked?.parse(src) ?? `<pre>${esc(src)}</pre>`

// ── Entry point — dispatches to list or detail ─────────────────────────────

export async function renderHelp({ query = {} } = {}) {
  if (query.topic) {
    await renderHelpDetail(query.topic)
  } else {
    await renderHelpList()
  }
}

// ── Help list ──────────────────────────────────────────────────────────────

async function renderHelpList() {
  setBreadcrumb([{ label: 'Help' }])
  setApp(`<div class="loading-state" aria-busy="true">Loading help topics…</div>`)

  let topics = []
  try {
    const ns     = getActiveNamespace()
    const all    = await searchDocuments({ all: [] })
    const docs   = Array.isArray(all) ? all : []
    topics = docs
      .filter(d =>
        d.kind === 'core/HelpTopic:1.0' &&
        d.spec?.listable !== false &&
        // show global topics always; also show namespace-scoped topics for active ns
        (d.metadata?.namespace == null || d.metadata?.namespace === ns)
      )
      .sort((a, b) => (a.spec?.title ?? a.name).localeCompare(b.spec?.title ?? b.name))
  } catch (e) {
    toastError(`Failed to load help topics: ${e.message}`)
  }

  if (topics.length === 0) {
    setApp(`
      <div class="page-title-row">
        <h1 class="page-title">Help</h1>
      </div>
      <div class="empty-state">
        <h3>No help topics available</h3>
        <p>No <code>core/HelpTopic:1.0</code> documents were found in the catalog.</p>
      </div>`)
    return
  }

  const cards = topics.map(d => {
    const title = esc(d.spec?.title ?? d.name)
    const desc  = esc(d.metadata?.description ?? d.spec?.content?.slice(0, 120) ?? '')
    return `
      <a class="help-card" href="#" data-topic="${esc(d.name)}">
        <div class="help-card-title">${title}</div>
        ${desc ? `<div class="help-card-desc">${desc}</div>` : ''}
      </a>`
  }).join('')

  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">Help</h1>
    </div>
    <div class="help-card-grid">${cards}</div>
  `)

  document.querySelectorAll('.help-card[data-topic]').forEach(el => {
    el.addEventListener('click', e => {
      e.preventDefault()
      navigate(`#/help?topic=${encodeURIComponent(el.dataset.topic)}`)
    })
  })
}

// ── Help detail ────────────────────────────────────────────────────────────

async function renderHelpDetail(topicName) {
  setBreadcrumb([
    { label: 'Help', href: '#/help' },
    { label: '…' },
  ])
  setApp(`<div class="loading-state" aria-busy="true">Loading…</div>`)

  let doc = null
  try {
    const all  = await searchDocuments({ all: [] })
    const docs = Array.isArray(all) ? all : []
    doc = docs.find(d => d.kind === 'core/HelpTopic:1.0' && d.name === topicName) ?? null
  } catch (e) {
    toastError(`Failed to load topic: ${e.message}`)
  }

  if (!doc) {
    setBreadcrumb([{ label: 'Help', href: '#/help' }, { label: 'Not found' }])
    setApp(`
      <div class="empty-state">
        <h3>Topic not found</h3>
        <p>No help topic named <strong>${esc(topicName)}</strong> exists in the catalog.</p>
        <a href="#/help" class="btn btn-secondary btn-sm">← Back to Help</a>
      </div>`)
    return
  }

  const title   = doc.spec?.title ?? doc.name
  const content = doc.spec?.content ?? ''

  setBreadcrumb([
    { label: 'Help', href: '#/help' },
    { label: title },
  ])

  // Convert #/help/foo-style md links to ?topic= query style so the router handles them
  const htmlContent = md(content).replace(
    /href="#\/help\/([^"]+)"/g,
    (_, name) => `href="#/help?topic=${encodeURIComponent(name)}"`
  )

  setApp(`
    <div class="page-title-row">
      <h1 class="page-title">${esc(title)}</h1>
      <div class="page-title-actions">
        <a href="#/help" class="btn btn-secondary btn-sm">← All Topics</a>
      </div>
    </div>
    <article class="help-prose">${htmlContent}</article>
  `)

  // Intercept internal help links rendered inside the Markdown
  document.querySelectorAll('.help-prose a[href^="#/help"]').forEach(el => {
    el.addEventListener('click', e => {
      e.preventDefault()
      navigate(el.getAttribute('href'))
    })
  })
}

