// ── Hash-based router ─────────────────────────────────────────────────────────
//
// Usage:
//   route('/namespaces/:ns/workflows/:id', ({ ns, id }) => render(ns, id))
//   start()

const routes = []
let currentCleanup = null

/** Register a route handler. Patterns use :param segments. */
export function route(pattern, handler) {
  const parts = pattern.split('/').filter(Boolean)
  routes.push({ parts, handler })
}

/**
 * Register a cleanup function that will be called before the next navigation.
 * Use this to stop intervals, cancel subscriptions, etc.
 */
export function setCleanup(fn) {
  currentCleanup = fn
}

/** Programmatically navigate to a hash path. */
export function navigate(path) {
  window.location.hash = path
}

function match(patternParts, hashParts) {
  if (patternParts.length !== hashParts.length) return null
  const params = {}
  for (let i = 0; i < patternParts.length; i++) {
    if (patternParts[i].startsWith(':')) {
      params[patternParts[i].slice(1)] = decodeURIComponent(hashParts[i])
    } else if (patternParts[i] !== hashParts[i]) {
      return null
    }
  }
  return params
}

function dispatch() {
  if (currentCleanup) {
    try { currentCleanup() } catch (_) {}
    currentCleanup = null
  }

  const raw   = window.location.hash.slice(1) || '/'
  const qIdx  = raw.indexOf('?')
  const path  = qIdx === -1 ? raw : raw.slice(0, qIdx)
  const query = qIdx === -1 ? {} : Object.fromEntries(new URLSearchParams(raw.slice(qIdx + 1)))
  const parts = path.split('/').filter(Boolean)

  for (const { parts: pattern, handler } of routes) {
    // Root route: both empty
    if (pattern.length === 0 && parts.length === 0) {
      handler({ query })
      return
    }
    const params = match(pattern, parts)
    if (params !== null) {
      handler({ ...params, query })
      return
    }
  }

  document.getElementById('app').innerHTML =
    '<div class="empty-state"><h2>404</h2><p>Page not found.</p></div>'
}

/** Start the router. Call once at app init. */
export function start() {
  window.addEventListener('hashchange', dispatch)
  dispatch()
}
