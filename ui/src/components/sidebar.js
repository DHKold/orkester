// ── Navigation sidebar ─────────────────────────────────────────────────────
//
// Responsibilities:
//   - Display mock user (avatar, name, team)
//   - Namespace selector dropdown (loaded from API)
//   - Collapsible sidebar (icon-only or full)
//   - Resizable when expanded (drag edge, persisted to localStorage)
//   - Main nav links with active state and Workspace group expansion

import { searchDocuments }                          from '../api.js'
import { MOCK_USER, getActiveNamespace, setActiveNamespace,
         isSidebarCollapsed, setSidebarCollapsed } from '../state.js'

const SIDEBAR_WIDTH_KEY = 'orkester-sidebar-width'
const MIN_WIDTH         = 160
const MAX_WIDTH         = 480

let cachedNamespaces = []

// ── Nav structure ──────────────────────────────────────────────────────────
// id is used to detect active group; href is the hash path; children = sub-items.
const NAV = [
  {
    id: 'dashboard', label: 'Dashboard', href: '#/',
    icon: `<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/>
      <rect x="3" y="14" width="7" height="9"/><rect x="14" y="14" width="7" height="9"/>
    </svg>`,
  },
  {
    id: 'catalog', label: 'Catalog', href: '#/catalog',
    icon: `<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/>
      <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>
    </svg>`,
  },
  {
    id: 'workspace', label: 'Workspace', group: true,
    icon: `<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>
    </svg>`,
    children: [
      {
        id: 'runners', label: 'Runners', href: '#/workspace/runners',
        icon: `<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8m-4-4v4"/>
        </svg>`,
      },
      {
        id: 'work-runs', label: 'Work Runs', href: '#/workspace/work-runs',
        icon: `<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polygon points="5 3 19 12 5 21 5 3"/>
        </svg>`,
      },
      {
        id: 'crons', label: 'Crons', href: '#/workspace/crons',
        icon: `<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>
        </svg>`,
      },
    ],
  },
  {
    id: 'metrics', label: 'Metrics', href: '#/metrics',
    icon: `<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/>
      <line x1="6" y1="20" x2="6" y2="14"/>
    </svg>`,
  },
  {
    id: 'settings', label: 'Settings', href: '#/settings',
    icon: `<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <circle cx="12" cy="12" r="3"/>
      <path d="M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14M12 1v2m0 18v2M4.22 4.22l1.42 1.42m12.72 12.72 1.42 1.42M1 12h2m18 0h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/>
    </svg>`,
  },
  {
    id: 'help', label: 'Help', href: '#/help',
    icon: `<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <circle cx="12" cy="12" r="10"/>
      <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><line x1="12" y1="17" x2="12.01" y2="17"/>
    </svg>`,
  },
]

// ── Public init ────────────────────────────────────────────────────────────

export async function initSidebar() {
  applyCollapsedState()
  renderUser()
  renderNav()
  initCollapseButton()
  initResizer()
  initNamespaceSelector()

  // Re-highlight active nav item on every navigation
  window.addEventListener('hashchange', updateActiveNav)

  // Refresh namespace list when namespace changes externally
  window.addEventListener('orkester:namespace-changed', () => {
    renderNamespaceLabel()
  })

  // Load namespaces from API (non-blocking)
  try {
    const data = await searchDocuments({ all: [{ when: { eq: { lhs: { field: 'kind' }, rhs: { value: 'workaholic/Namespace:1.0' } } } }] })
    cachedNamespaces = data ?? []
  } catch (_) {
    cachedNamespaces = []
  }
  renderNamespaceDropdown()
  renderNamespaceLabel()
}

// ── User section ───────────────────────────────────────────────────────────

function renderUser() {
  const avatar = document.getElementById('sidebar-avatar')
  const name   = document.getElementById('sidebar-user-name')
  const team   = document.getElementById('sidebar-user-team')
  if (!avatar) return
  avatar.textContent = MOCK_USER.initials
  name.textContent   = MOCK_USER.name
  team.innerHTML = MOCK_USER.team
    ? `<span class="sidebar-team-dot" style="background:${MOCK_USER.team.color}"></span>
       <span>${MOCK_USER.team.name}</span>`
    : ''
}

// ── Namespace selector ─────────────────────────────────────────────────────

function initNamespaceSelector() {
  const btn      = document.getElementById('sidebar-ns-btn')
  const dropdown = document.getElementById('sidebar-ns-dropdown')
  if (!btn) return

  btn.addEventListener('click', (e) => {
    e.stopPropagation()
    const open = !dropdown.classList.contains('hidden')
    dropdown.classList.toggle('hidden', open)
    btn.setAttribute('aria-expanded', String(!open))
  })

  // Close on outside click
  document.addEventListener('click', () => {
    dropdown.classList.add('hidden')
    btn.setAttribute('aria-expanded', 'false')
  })
}

function renderNamespaceDropdown() {
  const dropdown = document.getElementById('sidebar-ns-dropdown')
  if (!dropdown) return
  const active = getActiveNamespace()

  const globalOpt = `
    <div class="sidebar-ns-option${active === null ? ' selected' : ''}"
         role="option" data-ns="" aria-selected="${active === null}">
      <span class="sidebar-ns-option-dot" style="background:var(--text-3)"></span>
      <em>Global (all namespaces)</em>
    </div>
  `

  dropdown.innerHTML = globalOpt + (cachedNamespaces.map(ns => `
    <div class="sidebar-ns-option${ns.name === active ? ' selected' : ''}"
         role="option" data-ns="${ns.name}" aria-selected="${ns.name === active}">
      <span class="sidebar-ns-option-dot"></span>
      ${ns.name}
    </div>
  `).join('') || '<div class="sidebar-ns-option" style="opacity:0.5;padding-left:2rem">No namespaces found</div>')

  dropdown.querySelectorAll('.sidebar-ns-option[data-ns]').forEach(el => {
    el.addEventListener('click', (e) => {
      e.stopPropagation()
      setActiveNamespace(el.dataset.ns)
      renderNamespaceDropdown()
      renderNamespaceLabel()
      dropdown.classList.add('hidden')
      document.getElementById('sidebar-ns-btn')?.setAttribute('aria-expanded', 'false')
      // Reload the current page so it reflects the new namespace
      window.dispatchEvent(new HashChangeEvent('hashchange'))
    })
  })
}

function renderNamespaceLabel() {
  const label = document.getElementById('sidebar-ns-label')
  if (label) label.textContent = getActiveNamespace() || 'Global'
}

// ── Nav rendering ──────────────────────────────────────────────────────────

function currentPath() {
  const hash = window.location.hash.slice(1) || '/'
  return hash.split('?')[0]
}

function isGroupActive(group) {
  const path = currentPath()
  return group.children.some(c => path === c.href.slice(1) || path.startsWith(c.href.slice(1) + '/'))
}

function renderNav() {
  const nav = document.getElementById('sidebar-nav')
  if (!nav) return
  const path = currentPath()

  nav.innerHTML = NAV.map(item => {
    if (item.group) {
      const active  = isGroupActive(item)
      const isOpen  = active  // auto-open when a child is active
      return `
        <button class="nav-item group-header ${isOpen ? 'group-open' : ''}"
                data-group="${item.id}" aria-expanded="${isOpen}">
          ${item.icon}
          <span class="nav-label">${item.label}</span>
          <svg class="nav-group-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M9 18l6-6-6-6"/>
          </svg>
        </button>
        <div class="nav-group-children${isOpen ? ' open' : ''}" data-children="${item.id}">
          ${item.children.map(child => `
            <a class="nav-item sub ${path === child.href.slice(1) ? 'active' : ''}"
               href="${child.href}">
              ${child.icon}
              <span class="nav-label">${child.label}</span>
            </a>
          `).join('')}
        </div>
      `
    }
    const active = item.href === '#/'
      ? path === '/'
      : path === item.href.slice(1) || path.startsWith(item.href.slice(1) + '/')
    return `
      <a class="nav-item${active ? ' active' : ''}" href="${item.href}">
        ${item.icon}
        <span class="nav-label">${item.label}</span>
      </a>
    `
  }).join('')

  // Wire up group toggles
  nav.querySelectorAll('.nav-item.group-header').forEach(btn => {
    btn.addEventListener('click', () => {
      const id       = btn.dataset.group
      const children = nav.querySelector(`[data-children="${id}"]`)
      const isOpen   = children.classList.toggle('open')
      btn.classList.toggle('group-open', isOpen)
      btn.setAttribute('aria-expanded', String(isOpen))
    })
  })
}

function updateActiveNav() {
  renderNav()
}

// ── Collapse toggle ────────────────────────────────────────────────────────

function applyCollapsedState() {
  document.body.classList.toggle('sidebar-collapsed', isSidebarCollapsed())
}

function initCollapseButton() {
  const btn = document.getElementById('sidebar-collapse-btn')
  if (!btn) return
  btn.addEventListener('click', () => {
    const collapsed = !document.body.classList.contains('sidebar-collapsed')
    document.body.classList.toggle('sidebar-collapsed', collapsed)
    setSidebarCollapsed(collapsed)
  })
}

// ── Drag-to-resize ─────────────────────────────────────────────────────────

function initResizer() {
  const sidebar = document.getElementById('sidebar')
  const resizer = document.getElementById('sidebar-resizer')
  if (!resizer || !sidebar) return

  // Restore saved width
  const saved = parseInt(localStorage.getItem(SIDEBAR_WIDTH_KEY), 10)
  if (saved >= MIN_WIDTH && saved <= MAX_WIDTH) {
    document.body.style.setProperty('--sidebar-width', `${saved}px`)
  }

  resizer.addEventListener('mousedown', (e) => {
    if (document.body.classList.contains('sidebar-collapsed')) return
    e.preventDefault()
    resizer.classList.add('dragging')
    document.body.style.userSelect = 'none'
    document.body.style.cursor     = 'col-resize'

    const onMove = (ev) => {
      const w = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, ev.clientX))
      document.body.style.setProperty('--sidebar-width', `${w}px`)
    }

    const onUp = () => {
      resizer.classList.remove('dragging')
      document.body.style.userSelect = ''
      document.body.style.cursor     = ''
      const current = parseInt(
        getComputedStyle(document.body).getPropertyValue('--sidebar-width'), 10
      )
      if (current) localStorage.setItem(SIDEBAR_WIDTH_KEY, current)
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup',   onUp)
    }

    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup',   onUp)
  })
}
