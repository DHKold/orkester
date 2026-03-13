import { listCrons, deleteCron, createCron, updateCron, listWorks, getWork } from '../api.js'
import { esc, fmtDateShort, fmtDate, badge, setApp, breadcrumb, renderKvEditor, readKv, kvToObject, applyFilter, applySort, paginate, pagerHTML } from '../utils.js'
import { toastError, toastSuccess } from '../components/toast.js'
import { openModal, closeModal } from '../components/modal.js'

let allCrons  = []
let cronState = { q: '', sortKey: 'created_at', sortDir: 'desc', page: 1 }

export async function renderCrons({ ns }) {
  cronState = { q: '', sortKey: 'created_at', sortDir: 'desc', page: 1 }
  allCrons  = []

  setApp(`
    ${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns,href:`#/namespaces/${encodeURIComponent(ns)}`},{label:'Crons'}])}
    <div class="row-between" style="margin-bottom:1rem">
      <div class="row" style="gap:0.75rem;align-items:center">
        <h3 style="margin:0">Crons <span class="muted" id="crons-count" style="font-size:0.85rem"></span></h3>
        <input type="search" id="crons-filter" placeholder="Filter…" class="list-filter" />
      </div>
      <button id="btn-new-cron" class="outline btn-sm">New Cron</button>
    </div>
    <div id="crons-list"><p aria-busy="true">Loading crons…</p></div>
  `)

  document.getElementById('crons-filter').addEventListener('input', (e) => {
    cronState.q = e.target.value; cronState.page = 1; drawCrons(ns)
  })
  document.getElementById('btn-new-cron').addEventListener('click', () => openCronModal(ns, null))

  await fetchAndDraw(ns)
}

async function fetchAndDraw(ns) {
  try {
    const data = await listCrons(ns)
    allCrons   = data.crons ?? []
    drawCrons(ns)
  } catch (e) {
    toastError(`Failed to load crons: ${e.message}`)
    const el = document.getElementById('crons-list')
    if (el) el.innerHTML = '<div class="empty-state"><p>Failed to load crons.</p></div>'
  }
}

function drawCrons(ns) {
  const el = document.getElementById('crons-list')
  if (!el) return

  const SORT_FNS = {
    id:         c => c.id,
    schedule:   c => c.schedule,
    work:       c => c.work_name,
    created_at: c => c.created_at,
  }

  const filtered = applyFilter(allCrons, cronState.q, c => c.id, c => c.work_name, c => c.description ?? '')
  const sorted   = applySort(filtered, SORT_FNS[cronState.sortKey], cronState.sortDir)
  const { slice, page, pages, total } = paginate(sorted, cronState.page)
  cronState.page = page

  const countEl = document.getElementById('crons-count')
  if (countEl) countEl.textContent = total < allCrons.length ? `(${total} of ${allCrons.length})` : `(${total})`

  if (allCrons.length === 0) {
    el.innerHTML = '<div class="empty-state"><p>No crons yet. Create one to schedule recurring workflows.</p></div>'
    return
  }
  if (filtered.length === 0) {
    el.innerHTML = '<div class="empty-state"><p>No crons match the current filter.</p></div>'
    return
  }

  const sortInd   = k => k === cronState.sortKey ? (cronState.sortDir === 'asc' ? ' ▲' : ' ▼') : ' ⇅'
  const activeCls = k => k === cronState.sortKey ? ' sort-active' : ''

  const rows = slice.map(c => {
    const enabledBadge = c.enabled
      ? '<span class="badge badge--succeeded">enabled</span>'
      : '<span class="badge badge--cancelled">disabled</span>'
    return `
      <tr>
        <td><strong>${esc(c.id)}</strong>${c.description ? `<br><span class="muted">${esc(c.description)}</span>` : ''}</td>
        <td><code>${esc(c.schedule)}</code></td>
        <td><span>${esc(c.work_name)}</span><span class="muted"> @ ${esc(c.work_version)}</span></td>
        <td>${enabledBadge}</td>
        <td class="muted">${c.next_fire_at  ? fmtDateShort(c.next_fire_at)  : '—'}</td>
        <td class="muted">${c.last_fired_at ? fmtDateShort(c.last_fired_at) : '—'}</td>
        <td><div style="display:flex;gap:0.4rem">
          <button class="outline btn-xs" data-edit="${esc(c.id)}">Edit</button>
          <button class="outline btn-xs" data-toggle="${esc(c.id)}" data-enabled="${c.enabled}">${c.enabled ? 'Disable' : 'Enable'}</button>
          <button class="secondary outline btn-xs" data-delete="${esc(c.id)}">Delete</button>
        </div></td>
      </tr>`
  }).join('')

  el.innerHTML = `
    <figure><table>
      <thead><tr>
        <th class="sortable${activeCls('id')}" data-sort="id">ID / Description<span class="sort-ind">${sortInd('id')}</span></th>
        <th class="sortable${activeCls('schedule')}" data-sort="schedule">Schedule<span class="sort-ind">${sortInd('schedule')}</span></th>
        <th class="sortable${activeCls('work')}" data-sort="work">Work<span class="sort-ind">${sortInd('work')}</span></th>
        <th>Status</th><th>Next fire</th><th>Last fire</th><th></th>
      </tr></thead>
      <tbody>${rows}</tbody>
    </table></figure>
    <div id="crons-pager">${pagerHTML(page, pages, total)}</div>
  `

  el.querySelectorAll('th[data-sort]').forEach(th =>
    th.addEventListener('click', () => {
      const k = th.dataset.sort
      if (cronState.sortKey === k) cronState.sortDir = cronState.sortDir === 'asc' ? 'desc' : 'asc'
      else { cronState.sortKey = k; cronState.sortDir = 'asc' }
      cronState.page = 1; drawCrons(ns)
    })
  )
  el.querySelectorAll('[data-page]').forEach(btn =>
    btn.addEventListener('click', () => { cronState.page = +btn.dataset.page; drawCrons(ns) })
  )
  el.querySelectorAll('[data-edit]').forEach(btn =>
    btn.addEventListener('click', () => { const cron = allCrons.find(c => c.id === btn.dataset.edit); if (cron) openCronModal(ns, cron) })
  )
  el.querySelectorAll('[data-toggle]').forEach(btn =>
    btn.addEventListener('click', async () => {
      const enabled = btn.dataset.enabled === 'true'
      try { await updateCron(ns, btn.dataset.toggle, { enabled: !enabled }); toastSuccess(`Cron ${!enabled ? 'enabled' : 'disabled'}.`); await fetchAndDraw(ns) }
      catch (e) { toastError(e.message) }
    })
  )
  el.querySelectorAll('[data-delete]').forEach(btn =>
    btn.addEventListener('click', async () => {
      if (!confirm(`Delete cron "${btn.dataset.delete}"?`)) return
      try { await deleteCron(ns, btn.dataset.delete); toastSuccess('Cron deleted.'); await fetchAndDraw(ns) }
      catch (e) { toastError(e.message) }
    })
  )
}

// ── Create / Edit Cron Modal ──────────────────────────────────────────────────

async function openCronModal(ns, existing = null) {
  const isEdit = !!existing
  openModal(isEdit ? `Edit Cron: ${existing.id}` : 'New Cron', '<p aria-busy="true">Loading…</p>')

  try {
    const data  = await listWorks(ns)
    const works = data.works ?? []

    const selectedWork  = isEdit ? `${existing.work_name}|${existing.work_version}` : ''
    const workOptions   = works.map(w => {
      const val = `${w.name}|${w.version}`
      return `<option value="${esc(val)}" ${val === selectedWork ? 'selected' : ''}>${esc(w.name)} @ ${esc(w.version)}</option>`
    }).join('')

    const html = `
      <form id="form-cron">
        <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.75rem">
          <label>ID
            <input type="text" id="cron-id" value="${esc(existing?.id ?? '')}"
              ${isEdit ? 'readonly' : 'required'} placeholder="nightly-etl" />
          </label>
          <label>Schedule
            <input type="text" id="cron-schedule" required
              value="${esc(existing?.schedule ?? '')}" placeholder="0 1 * * *" />
            <small>5-field cron expression (min hour dom mon dow)</small>
          </label>
        </div>

        <label>Description
          <input type="text" id="cron-desc" value="${esc(existing?.description ?? '')}" placeholder="Optional description" />
        </label>

        <label>Work
          <select id="cron-work" required>
            <option value="">Select a Work…</option>
            ${workOptions}
          </select>
        </label>

        <fieldset>
          <legend>Parameters</legend>
          <div id="cron-context"></div>
        </fieldset>

        <label style="display:flex;align-items:center;gap:0.5rem;cursor:pointer">
          <input type="checkbox" id="cron-enabled" ${existing?.enabled !== false ? 'checked' : ''} role="switch" />
          Enabled
        </label>

        <details>
          <summary class="muted" style="font-size:0.88rem">Concurrency policy</summary>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.5rem;margin-top:0.5rem">
            ${concurrencySelect('cron-on-running', 'When running', existing?.concurrency_policy?.on_running ?? 'skip')}
            ${concurrencySelect('cron-on-waiting', 'When waiting', existing?.concurrency_policy?.on_waiting ?? 'skip')}
            ${concurrencySelect('cron-on-paused',  'When paused',  existing?.concurrency_policy?.on_paused  ?? 'skip')}
            ${concurrencySelect('cron-on-default', 'Default',      existing?.concurrency_policy?.default_action ?? 'allow')}
          </div>
        </details>

        <div style="display:flex;gap:0.5rem;justify-content:flex-end;margin-top:1rem">
          <button type="button" class="secondary outline" id="cron-cancel-btn">Cancel</button>
          <button type="submit" id="cron-submit-btn">${isEdit ? 'Save Changes' : 'Create Cron'}</button>
        </div>
      </form>
    `

    document.getElementById('modal-body').innerHTML = html

    // Load context fields based on selected work
    const workSelect = document.getElementById('cron-work')
    const initCtx    = isEdit ? (existing.work_context ?? {}) : {}

    const loadContext = async (val) => {
      if (!val) { renderKvEditor('cron-context', initCtx, {}); return }
      const [wn, wv] = val.split('|')
      try {
        const w = await getWork(ns, wn, wv)
        renderKvEditor('cron-context', initCtx, w.spec?.inputs ?? {})
      } catch (_) { renderKvEditor('cron-context', initCtx, {}) }
    }

    workSelect.addEventListener('change', (e) => loadContext(e.target.value))
    await loadContext(workSelect.value)

    document.getElementById('cron-cancel-btn').addEventListener('click', closeModal)

    document.getElementById('form-cron').addEventListener('submit', async (e) => {
      e.preventDefault()
      const [wn, wv] = (workSelect.value || '').split('|')
      if (!wn) { toastError('Please select a Work.'); return }

      const submit = document.getElementById('cron-submit-btn')
      submit.setAttribute('aria-busy', 'true')
      submit.disabled = true

      try {
        const body = {
          id:           document.getElementById('cron-id').value.trim(),
          description:  document.getElementById('cron-desc').value.trim(),
          schedule:     document.getElementById('cron-schedule').value.trim(),
          work_name:    wn,
          work_version: wv,
          work_context: kvToObject(readKv('cron-context')),
          enabled:      document.getElementById('cron-enabled').checked,
          concurrency_policy: {
            on_running:     document.getElementById('cron-on-running').value,
            on_waiting:     document.getElementById('cron-on-waiting').value,
            on_paused:      document.getElementById('cron-on-paused').value,
            default_action: document.getElementById('cron-on-default').value,
          }
        }

        if (isEdit) {
          await updateCron(ns, existing.id, body)
          toastSuccess('Cron updated.')
        } else {
          await createCron(ns, body)
          toastSuccess('Cron created.')
        }
        closeModal()
        await fetchAndDraw(ns)
      } catch (err) {
        toastError(`Failed to save cron: ${err.message}`)
      } finally {
        submit.removeAttribute('aria-busy')
        submit.disabled = false
      }
    })

  } catch (e) {
    document.getElementById('modal-body').innerHTML =
      `<p class="muted">Failed to load: ${esc(e.message)}</p>`
    toastError(e.message)
  }
}

function concurrencySelect(id, label, selected) {
  const opts = ['allow', 'skip', 'replace', 'cancel_existing']
  return `
    <label>${esc(label)}
      <select id="${id}">
        ${opts.map(o => `<option value="${o}" ${o === selected ? 'selected' : ''}>${esc(o)}</option>`).join('')}
      </select>
    </label>
  `
}
