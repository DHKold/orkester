import { listNamespaces } from '../api.js'

const STORAGE_KEY = 'orkester-sidebar-width'
const MIN_WIDTH   = 140
const MAX_WIDTH   = 480

let cachedNamespaces = []

/** Load namespaces and render sidebar. Call once at startup. */
export async function initSidebar() {
  try {
    const data = await listNamespaces()
    cachedNamespaces = data.namespaces ?? []
  } catch (_) {
    cachedNamespaces = []
  }
  initResizer()
  renderSidebar()
  window.addEventListener('hashchange', updateSidebarActive)
}

function initResizer() {
  const saved = parseInt(localStorage.getItem(STORAGE_KEY), 10)
  if (saved >= MIN_WIDTH && saved <= MAX_WIDTH) {
    document.body.style.setProperty('--sidebar-width', `${saved}px`)
  }

  const resizer = document.getElementById('sidebar-resizer')
  if (!resizer) return

  resizer.addEventListener('mousedown', (e) => {
    e.preventDefault()
    resizer.classList.add('dragging')
    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'col-resize'

    const onMove = (ev) => {
      const w = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, ev.clientX))
      document.body.style.setProperty('--sidebar-width', `${w}px`)
    }

    const onUp = () => {
      resizer.classList.remove('dragging')
      document.body.style.userSelect = ''
      document.body.style.cursor = ''
      const current = parseInt(
        getComputedStyle(document.body).getPropertyValue('--sidebar-width'), 10
      )
      if (current) localStorage.setItem(STORAGE_KEY, current)
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup',   onUp)
    }

    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup',   onUp)
  })
}

function renderSidebar() {
  const nav = document.getElementById('sidebar-nav')
  if (!nav) return

  const nsLinks = cachedNamespaces.map(ns => {
    const nsEnc = encodeURIComponent(ns.name)
    return `<a href="#/namespaces/${nsEnc}" class="sidebar-ns-link" data-ns="${ns.name}">
      <span class="sidebar-ns-dot"></span>${ns.name}
    </a>`
  }).join('')

  nav.innerHTML = `
    <div class="sidebar-section">
      <a href="#/namespaces" class="sidebar-link" data-path="/namespaces">
        <span>🗂</span> Namespaces
      </a>
      <a href="#/health" class="sidebar-link" data-path="/health">
        <span>💚</span> Health
      </a>
      <a href="#/metrics" class="sidebar-link" data-path="/metrics">
        <span>📊</span> Metrics
      </a>
    </div>
    ${cachedNamespaces.length > 0 ? `
      <div class="sidebar-divider"></div>
      <div class="sidebar-section-title">Namespaces</div>
      <div class="sidebar-section sidebar-ns-section">
        ${nsLinks}
      </div>
    ` : ''}
    <div id="sidebar-subnav"></div>
  `
  updateSidebarActive()
}

/** Update active highlights and contextual sub-nav based on current hash. */
export function updateSidebarActive() {
  const raw  = window.location.hash.slice(1) || '/'
  const qIdx = raw.indexOf('?')
  const path = qIdx === -1 ? raw : raw.slice(0, qIdx)

  // Highlight top-level links
  document.querySelectorAll('.sidebar-link[data-path]').forEach(a => {
    const p      = a.dataset.path
    const active = path === p || (p !== '/namespaces' && path.startsWith(p + '/'))
    a.classList.toggle('active', active)
  })

  // Highlight namespace pills
  document.querySelectorAll('.sidebar-ns-link[data-ns]').forEach(a => {
    const nsEnc = encodeURIComponent(a.dataset.ns)
    const inNs  = path === `/namespaces/${nsEnc}` || path.startsWith(`/namespaces/${nsEnc}/`)
    a.classList.toggle('active', inNs)
  })

  // Contextual sub-nav when inside a namespace
  const nsMatch = path.match(/^\/namespaces\/([^/]+)(?:\/.*)?$/)
  renderSubNav(nsMatch ? decodeURIComponent(nsMatch[1]) : null, path)
}

function renderSubNav(ns, path) {
  const el = document.getElementById('sidebar-subnav')
  if (!el) return

  if (!ns) {
    el.innerHTML = ''
    return
  }

  const nsEnc = encodeURIComponent(ns)
  const tabs = [
    { label: 'Catalog',   p: `/namespaces/${nsEnc}` },
    { label: 'Workflows', p: `/namespaces/${nsEnc}/workflows` },
    { label: 'Crons',     p: `/namespaces/${nsEnc}/crons` },
  ]

  el.innerHTML = `
    <div class="sidebar-divider"></div>
    <div class="sidebar-section-title">${ns}</div>
    <div class="sidebar-section">
      ${tabs.map(t => {
        // Catalog matches exactly; Workflows/Crons also match child paths (e.g. workflow detail)
        const active = t.label === 'Catalog'
          ? path === t.p
          : path === t.p || path.startsWith(t.p + '/')
        return `<a href="#${t.p}" class="sidebar-link sidebar-tab${active ? ' active' : ''}">${t.label}</a>`
      }).join('')}
    </div>
  `
}
