// ── Reusable data table ────────────────────────────────────────────────────
//
// Usage (inside a page render function):
//
//   const html = renderTable({
//     columns: [
//       { key: 'name',   label: 'Name',   sortable: true },
//       { key: 'status', label: 'Status', render: (row) => badge(row.status) },
//     ],
//     rows:        pagedRows,      // already-paginated rows
//     total:       allRows.length,
//     page:        state.page,
//     pageSize:    25,
//     sortKey:     state.sortKey,
//     sortDir:     state.sortDir,
//     emptyMsg:    'No items found.',
//   })
//   document.getElementById('table-wrap').innerHTML = html
//   bindTable('table-wrap', {
//     onSort:     (key, dir) => { state.sortKey = key; state.sortDir = dir; re-render() },
//     onPage:     (page)     => { state.page    = page; re-render() },
//     onRowClick: (row)      => navigate(`#/.../${row.name}`),
//     rows:       pagedRows, // needed for onRowClick lookup
//   })

import { esc } from '../utils.js'

const PAGE_SIZE = 25

/**
 * Generate the HTML for a sortable, paginated data table.
 *
 * @param {Object}   opts
 * @param {Array}    opts.columns   - Column definitions
 * @param {Array}    opts.rows      - Current page rows (already paginated/sorted externally)
 * @param {number}   opts.total     - Total row count before pagination
 * @param {number}   [opts.page=1]
 * @param {number}   [opts.pageSize=25]
 * @param {string}   [opts.sortKey]
 * @param {string}   [opts.sortDir='asc']
 * @param {string}   [opts.emptyMsg='No items found.']
 * @param {boolean}  [opts.clickable=true]  - Whether rows are clickable
 * @returns {string} HTML string
 */
export function renderTable(opts) {
  const {
    columns,
    rows      = [],
    total     = rows.length,
    page      = 1,
    pageSize  = PAGE_SIZE,
    sortKey   = '',
    sortDir   = 'asc',
    emptyMsg  = 'No items found.',
    clickable = true,
  } = opts

  const totalPages = Math.max(1, Math.ceil(total / pageSize))
  const start      = (page - 1) * pageSize + 1
  const end        = Math.min(page * pageSize, total)

  const thead = `<thead><tr>${columns.map(col => {
    const isSorted = col.key === sortKey
    const sortClass = isSorted ? `sort-${sortDir}` : ''
    const noSort    = col.sortable === false ? 'no-sort' : ''
    return `<th class="${sortClass} ${noSort}" data-sort="${col.sortable !== false ? col.key : ''}" style="${col.width ? `width:${col.width}` : ''}">
      ${esc(col.label)}<span class="sort-ind"></span>
    </th>`
  }).join('')}</tr></thead>`

  const tbody = rows.length === 0
    ? `<tbody><tr><td colspan="${columns.length}" style="text-align:center;color:var(--text-3);padding:2rem">${esc(emptyMsg)}</td></tr></tbody>`
    : `<tbody>${rows.map((row, i) => `
        <tr class="${clickable ? 'clickable' : ''}" data-row-idx="${i}">
          ${columns.map(col => {
            const raw = col.key.split('.').reduce((o, k) => o?.[k], row)
            const cell = col.render ? col.render(row, raw) : esc(raw ?? '—')
            return `<td>${cell}</td>`
          }).join('')}
        </tr>
      `).join('')}</tbody>`

  const footer = `
    <div class="table-footer">
      <span>${total === 0 ? 'No results' : `${start}–${end} of ${total}`}</span>
      <div class="pagination">
        <button class="pagination-btn" data-page="${page - 1}" ${page <= 1 ? 'disabled' : ''}>‹</button>
        ${paginationPages(page, totalPages).map(p =>
          p === '…'
            ? `<span style="padding:0 0.25rem;color:var(--text-3)">…</span>`
            : `<button class="pagination-btn${p === page ? ' active' : ''}" data-page="${p}">${p}</button>`
        ).join('')}
        <button class="pagination-btn" data-page="${page + 1}" ${page >= totalPages ? 'disabled' : ''}>›</button>
      </div>
    </div>`

  return `<div class="table-scroll"><table class="data-table">${thead}${tbody}</table></div>${footer}`
}

/**
 * Bind click handlers to a rendered table.
 * Must be called after the table HTML is in the DOM.
 *
 * @param {string}   containerId  - ID of the .table-wrap element
 * @param {Object}   handlers
 * @param {Function} [handlers.onSort]     - (key, dir) => void
 * @param {Function} [handlers.onPage]     - (page: number) => void
 * @param {Function} [handlers.onRowClick] - (row: object) => void
 * @param {Array}    [handlers.rows]       - The rows array (needed for onRowClick)
 * @param {string}   [handlers.sortKey]    - Current sort key
 * @param {string}   [handlers.sortDir]    - Current sort dir
 */
export function bindTable(containerId, handlers) {
  const wrap = document.getElementById(containerId)
  if (!wrap) return

  const { onSort, onPage, onRowClick, rows = [], sortKey = '', sortDir = 'asc' } = handlers

  // Sort headers
  if (onSort) {
    wrap.querySelectorAll('th[data-sort]').forEach(th => {
      if (!th.dataset.sort) return
      th.addEventListener('click', () => {
        const key = th.dataset.sort
        const dir = key === sortKey && sortDir === 'asc' ? 'desc' : 'asc'
        onSort(key, dir)
      })
    })
  }

  // Pagination
  if (onPage) {
    wrap.querySelectorAll('.pagination-btn[data-page]').forEach(btn => {
      btn.addEventListener('click', () => {
        const p = parseInt(btn.dataset.page, 10)
        if (!isNaN(p)) onPage(p)
      })
    })
  }

  // Row click
  if (onRowClick) {
    wrap.querySelectorAll('tr[data-row-idx]').forEach(tr => {
      tr.addEventListener('click', () => {
        const idx = parseInt(tr.dataset.rowIdx, 10)
        if (rows[idx]) onRowClick(rows[idx])
      })
    })
  }
}

// ── Internal helpers ───────────────────────────────────────────────────────

function paginationPages(current, total) {
  if (total <= 7) return Array.from({ length: total }, (_, i) => i + 1)
  const pages = [1]
  if (current > 3)           pages.push('…')
  for (let p = Math.max(2, current - 1); p <= Math.min(total - 1, current + 1); p++) pages.push(p)
  if (current < total - 2)   pages.push('…')
  pages.push(total)
  return pages
}
