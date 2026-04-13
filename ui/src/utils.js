// ── HTML escaping ──────────────────────────────────────────────────────────

/** Escape a value for safe insertion into HTML. Always use this for user data. */
export function esc(v) {
  return String(v ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

// ── Date / time formatting ─────────────────────────────────────────────────

export function fmtDate(iso) {
  if (!iso) return '—'
  return new Date(iso).toLocaleString(undefined, {
    year: 'numeric', month: 'short', day: 'numeric',
    hour: '2-digit', minute: '2-digit', second: '2-digit',
  })
}

export function fmtDateShort(iso) {
  if (!iso) return '—'
  return new Date(iso).toLocaleString(undefined, {
    month: 'short', day: 'numeric',
    hour: '2-digit', minute: '2-digit',
  })
}

export function fmtDuration(startIso, endIso, durationSeconds) {
  if (durationSeconds != null) {
    const s = durationSeconds
    if (s < 1)   return `${(s * 1000).toFixed(0)}ms`
    if (s < 60)  return `${s.toFixed(2).replace(/\.?0+$/, '')}s`
    const m = Math.floor(s / 60), rs = s % 60
    if (m < 60)  return `${m}m ${Math.round(rs)}s`
    const h = Math.floor(m / 60), rm = m % 60
    return `${h}h ${rm}m`
  }
  if (!startIso) return '—'
  const ms = (endIso ? new Date(endIso) : new Date()) - new Date(startIso)
  if (ms < 0) return '—'
  return fmtDuration(null, null, ms / 1000)
}

export function fmtUptime(seconds) {
  if (seconds == null) return '—'
  const d = Math.floor(seconds / 86400)
  const h = Math.floor((seconds % 86400) / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  if (d > 0) return `${d}d ${h}h ${m}m`
  if (h > 0) return `${h}h ${m}m`
  if (m > 0) return `${m}m ${s}s`
  return `${s}s`
}

export function fmtRelative(iso) {
  if (!iso) return '—'
  const ms = Date.now() - new Date(iso).getTime()
  const s  = Math.floor(ms / 1000)
  if (s < 60)   return `${s}s ago`
  const m = Math.floor(s / 60)
  if (m < 60)   return `${m}m ago`
  const h = Math.floor(m / 60)
  if (h < 24)   return `${h}h ago`
  return fmtDateShort(iso)
}

// ── DOM helpers ────────────────────────────────────────────────────────────

/** Replace the #app element's inner HTML. */
export function setApp(html) {
  document.getElementById('app').innerHTML = html
}

/** Set the breadcrumb nav from an array of { label, href? } segments. */
export function setBreadcrumb(segments) {
  const nav = document.getElementById('breadcrumb')
  if (!nav) return
  const parts = segments.map((s, i) => {
    const isLast = i === segments.length - 1
    if (isLast || !s.href) return `<span class="${isLast ? 'current' : ''}">${esc(s.label)}</span>`
    return `<a href="${s.href}">${esc(s.label)}</a>`
  })
  nav.innerHTML = parts.join('<span class="sep">›</span>')
}

// ── List utilities ─────────────────────────────────────────────────────────

/** Filter an array of objects by a query string across all string values. */
export function applyFilter(items, q) {
  if (!q) return items
  const lq = q.toLowerCase()
  return items.filter(item =>
    Object.values(item).some(v => String(v ?? '').toLowerCase().includes(lq))
  )
}

/** Sort an array of objects by a dot-path key. */
export function applySort(items, key, dir) {
  if (!key) return items
  return [...items].sort((a, b) => {
    const av = deepGet(a, key) ?? ''
    const bv = deepGet(b, key) ?? ''
    const cmp = typeof av === 'number' && typeof bv === 'number'
      ? av - bv
      : String(av).localeCompare(String(bv))
    return dir === 'asc' ? cmp : -cmp
  })
}

/** Return the page slice. page is 1-based. */
export function paginate(items, page, pageSize = 25) {
  const start = (page - 1) * pageSize
  return items.slice(start, start + pageSize)
}

/** Access a nested property by dot-path, e.g. 'status.state'. */
export function deepGet(obj, path) {
  return path.split('.').reduce((o, k) => o?.[k], obj)
}

// ── Misc ───────────────────────────────────────────────────────────────────

/** Normalise a document name that may be a full ref like "ns/name:version". */
export function shortName(ref) {
  if (!ref) return '—'
  const slash = ref.lastIndexOf('/')
  const colon = ref.lastIndexOf(':')
  let name = ref
  if (slash >= 0) name = name.slice(slash + 1)
  if (colon >= 0) name = name.slice(0, name.lastIndexOf(':'))
  return name || ref
}

/** Returns a terminal state check (no further polling needed). */
export function isTerminal(state) {
  return ['succeeded', 'failed', 'cancelled', 'dropped', 'inactive'].includes(state)
}
