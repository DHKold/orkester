// ── Global application state ───────────────────────────────────────────────
//
// Lightweight state module – no framework, just exports and events.

const NS_KEY       = 'orkester-active-ns'
const SIDEBAR_KEY  = 'orkester-sidebar-collapsed'

// ── Mock user (replace with real auth in the future) ──────────────────────
export const MOCK_USER = {
  id:     'user:john',
  name:   'John Doe',
  initials: 'JD',
  team: {
    id:    'team:platform',
    name:  'Platform',
    color: '#3b82f6',
  },
}

// ── Active namespace ───────────────────────────────────────────────────────
let _activeNamespace = localStorage.getItem(NS_KEY) || null

export function getActiveNamespace() {
  return _activeNamespace
}

export function setActiveNamespace(ns) {
  _activeNamespace = ns || null
  if (ns) localStorage.setItem(NS_KEY, ns)
  else    localStorage.removeItem(NS_KEY)
  window.dispatchEvent(new CustomEvent('orkester:namespace-changed', { detail: ns }))
}

// ── Sidebar collapsed state ────────────────────────────────────────────────
export function isSidebarCollapsed() {
  return localStorage.getItem(SIDEBAR_KEY) === '1'
}

export function setSidebarCollapsed(val) {
  if (val) localStorage.setItem(SIDEBAR_KEY, '1')
  else     localStorage.removeItem(SIDEBAR_KEY)
}
