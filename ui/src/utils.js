// ── String escaping ───────────────────────────────────────────────────────────

/** Escape a value for safe insertion into HTML. */
export function esc(v) {
  return String(v ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

// ── Date / time formatting ────────────────────────────────────────────────────

export function fmtDate(iso) {
  if (!iso) return '—'
  return new Date(iso).toLocaleString(undefined, {
    year: 'numeric', month: 'short', day: 'numeric',
    hour: '2-digit', minute: '2-digit', second: '2-digit'
  })
}

export function fmtDateShort(iso) {
  if (!iso) return '—'
  return new Date(iso).toLocaleString(undefined, {
    month: 'short', day: 'numeric',
    hour: '2-digit', minute: '2-digit'
  })
}

export function fmtDuration(startIso, endIso) {
  if (!startIso) return '—'
  const ms = (endIso ? new Date(endIso) : new Date()) - new Date(startIso)
  if (ms < 0) return '—'
  const s = Math.floor(ms / 1000)
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60); const rs = s % 60
  if (m < 60) return `${m}m ${rs}s`
  const h = Math.floor(m / 60); const rm = m % 60
  return `${h}h ${rm}m`
}

export function fmtUptime(seconds) {
  if (!seconds && seconds !== 0) return '—'
  const d = Math.floor(seconds / 86400)
  const h = Math.floor((seconds % 86400) / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  if (d > 0) return `${d}d ${h}h ${m}m`
  if (h > 0) return `${h}h ${m}m`
  if (m > 0) return `${m}m ${s}s`
  return `${s}s`
}

// ── Status badge ──────────────────────────────────────────────────────────────

export function badge(status) {
  return `<span class="badge badge--${esc(status)}">${esc(status)}</span>`
}

// ── DOM helpers ───────────────────────────────────────────────────────────────

export function setApp(html) {
  document.getElementById('app').innerHTML = html
}

/**
 * Render a breadcrumb bar.
 * @param {Array<{label: string, href?: string}>} segments
 */
export function breadcrumb(segments) {
  const parts = segments.map((s, i) => {
    const isLast = i === segments.length - 1
    if (isLast || !s.href) return `<span>${esc(s.label)}</span>`
    return `<a href="${s.href}">${esc(s.label)}</a>`
  })
  return `<nav class="breadcrumb">${parts.join('<span class="sep">›</span>')}</nav>`
}

// ── KV pair editor ────────────────────────────────────────────────────────────

/**
 * Render a dynamic key-value editor.
 * @param {string} containerId - ID of the div to render into
 * @param {Record<string,string>} initial - initial pairs
 * @param {Record<string,string>} declared - declared keys from Work.spec.inputs (name → description)
 */
export function renderKvEditor(containerId, initial = {}, declared = {}) {
  const container = document.getElementById(containerId)
  if (!container) return

  const render = (pairs) => {
    const rows = pairs.map((pair, i) => `
      <div class="kv-row" data-idx="${i}">
        <input type="text" placeholder="key" value="${esc(pair.k)}" data-role="key" />
        <input type="text" placeholder="value" value="${esc(pair.v)}" data-role="val" />
        <button type="button" class="secondary outline btn-xs kv-remove">✕</button>
      </div>
    `).join('')

    const declaredHints = Object.keys(declared).length > 0
      ? `<p class="muted" style="margin-bottom:0.4rem">Declared inputs: ${Object.keys(declared).map(k => `<span class="tag">${esc(k)}</span>`).join(' ')}</p>`
      : ''

    container.innerHTML = `
      ${declaredHints}
      <div id="${containerId}-rows">${rows}</div>
      <button type="button" class="outline btn-xs" id="${containerId}-add">+ Add parameter</button>
    `

    document.getElementById(`${containerId}-add`).addEventListener('click', () => {
      const current = readKv(containerId)
      current.push({ k: '', v: '' })
      render(current)
    })
    container.querySelectorAll('.kv-remove').forEach(btn => {
      btn.addEventListener('click', () => {
        const idx = +btn.closest('.kv-row').dataset.idx
        const current = readKv(containerId)
        current.splice(idx, 1)
        render(current)
      })
    })
  }

  const initialPairs = Object.entries(initial).map(([k, v]) => ({ k, v }))
  // Ensure all declared keys have at least an empty row
  const declaredKeys = Object.keys(declared)
  for (const k of declaredKeys) {
    if (!initialPairs.find(p => p.k === k)) {
      initialPairs.push({ k, v: '' })
    }
  }
  render(initialPairs.length ? initialPairs : [{ k: '', v: '' }])
}

/** Read current key-value pairs from a KV editor container. */
export function readKv(containerId) {
  const container = document.getElementById(containerId)
  if (!container) return []
  return Array.from(container.querySelectorAll('.kv-row')).map(row => ({
    k: row.querySelector('[data-role="key"]')?.value?.trim() ?? '',
    v: row.querySelector('[data-role="val"]')?.value?.trim() ?? '',
  }))
}

/** Convert KV pairs array to a plain object, skipping empty keys. */
export function kvToObject(pairs) {
  const obj = {}
  for (const { k, v } of pairs) {
    if (k) obj[k] = v
  }
  return obj
}
