// ── Key-value pair editor ──────────────────────────────────────────────────
//
// Renders a dynamic list of key-value input rows.
//
// Usage:
//   document.getElementById('params-wrap').innerHTML = renderKvEditor('params-editor', initial, hints)
//   // ... later, to read values:
//   const values = readKvEditor('params-editor')

import { esc } from '../utils.js'

/**
 * Render a KV editor.
 * @param {string}              id       - The id to give the container div
 * @param {Record<string,string>} initial  - Initial key-value pairs
 * @param {Record<string,string>} [hints]  - Declared keys → description hints
 * @returns {string} HTML
 */
export function renderKvEditor(id, initial = {}, hints = {}) {
  const entries = Object.entries(initial)
  if (entries.length === 0) entries.push(['', ''])

  const hintList = Object.entries(hints).map(([k, v]) =>
    `<option value="${esc(k)}" title="${esc(v)}">${esc(k)}</option>`
  ).join('')
  const datalistId = id + '-hints'

  const rows = entries.map(([k, v]) => kvRow(k, v, datalistId)).join('')

  return `
    <datalist id="${datalistId}">${hintList}</datalist>
    <div class="kv-editor" id="${id}">
      ${rows}
      <div>
        <button type="button" class="btn btn-ghost btn-sm" data-kv-add="${id}">+ Add param</button>
      </div>
    </div>`
}

function kvRow(k = '', v = '', datalistId = '') {
  return `
    <div class="kv-row">
      <input type="text" placeholder="key"   value="${esc(k)}" list="${esc(datalistId)}" class="kv-key" />
      <input type="text" placeholder="value" value="${esc(v)}" class="kv-val" />
      <button type="button" class="btn btn-ghost btn-sm" data-kv-remove title="Remove">✕</button>
    </div>`
}

/**
 * Attach add/remove event listeners to a rendered KV editor.
 * Must be called after the HTML is in the DOM.
 * @param {string} id
 */
export function bindKvEditor(id) {
  const container = document.getElementById(id)
  if (!container) return

  container.addEventListener('click', (e) => {
    const addBtn    = e.target.closest(`[data-kv-add="${id}"]`)
    const removeBtn = e.target.closest('[data-kv-remove]')

    if (addBtn) {
      const rowsContainer = container.querySelector('.kv-editor') ?? container
      const newRow = document.createElement('div')
      newRow.innerHTML = kvRow('', '', id + '-hints')
      // Insert before the "Add" button row
      addBtn.parentElement.insertAdjacentElement('beforebegin', newRow.firstElementChild)
    }

    if (removeBtn) {
      const row = removeBtn.closest('.kv-row')
      if (row) row.remove()
    }
  })
}

/**
 * Read the current key-value pairs from a rendered KV editor.
 * Skips rows with empty keys.
 * @param {string} id
 * @returns {Record<string,string>}
 */
export function readKvEditor(id) {
  const container = document.getElementById(id)
  if (!container) return {}
  const result = {}
  container.querySelectorAll('.kv-row').forEach(row => {
    const k = row.querySelector('.kv-key')?.value?.trim()
    const v = row.querySelector('.kv-val')?.value?.trim()
    if (k) result[k] = v ?? ''
  })
  return result
}
