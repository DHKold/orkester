import { listCrons, registerCron, unregisterCron } from '../api.js'
import { esc, fmtDateShort, setApp, breadcrumb, renderKvEditor, readKv, applyFilter, applySort, paginate, pagerHTML } from '../utils.js'
import { toastError, toastSuccess } from '../components/toast.js'
import { openModal, closeModal } from '../components/modal.js'

let allCrons = []
let cronState = { q: '', sortKey: 'name', sortDir: 'asc', page: 1 }

export async function renderCrons({ ns }) {
  cronState = { q: '', sortKey: 'name', sortDir: 'asc', page: 1 }
  allCrons = []
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
  document.getElementById('crons-filter').addEventListener('input', e => {
    cronState.q = e.target.value; cronState.page = 1; drawCrons(ns)
  })
  document.getElementById('btn-new-cron').addEventListener('click', () => openCronModal(ns, null))
  await fetchAndDraw(ns)
}

async function fetchAndDraw(ns) {
  try {
    const data = await listCrons()
    allCrons = data.crons ?? []
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
  if (allCrons.length === 0) { el.innerHTML = '<div class="empty-state"><p>No crons yet.</p></div>'; return }
  const SORT = { name: c => c.name, work: c => c.spec?.work_ref, enabled: c => c.spec?.enabled }
  const filtered = applyFilter(allCrons, cronState.q, c => c.name, c => c.spec?.work_ref ?? '')
  const sorted   = applySort(filtered, SORT[cronState.sortKey] ?? SORT.name, cronState.sortDir)
  const { slice, page, pages, total } = paginate(sorted, cronState.page)
  cronState.page = page
  const countEl = document.getElementById('crons-count')
  if (countEl) countEl.textContent = total < allCrons.length ? `(${total} of ${allCrons.length})` : `(${total})`
  if (filtered.length === 0) { el.innerHTML = '<div class="empty-state"><p>No crons match the filter.</p></div>'; return }
  const sInd = k => k === cronState.sortKey ? (cronState.sortDir === 'asc' ? ' ▲' : ' ▼') : ' ⇅'
  const sCls = k => k === cronState.sortKey ? ' sort-active' : ''
  const rows = slice.map(c => {
    const enabled  = c.spec?.enabled !== false
    const scheds   = (c.spec?.schedules ?? []).join(', ') || '—'
    const nextAt   = c.status?.next_scheduled_time
    return `<tr>
      <td><strong>${esc(c.name)}</strong></td>
      <td><code>${esc(scheds)}</code></td>
      <td>${esc(c.spec?.work_ref ?? '—')}</td>
      <td>${enabled ? '<span class="badge badge--succeeded">enabled</span>' : '<span class="badge badge--cancelled">disabled</span>'}</td>
      <td class="muted">${nextAt ? fmtDateShort(nextAt) : '—'}</td>
      <td><div style="display:flex;gap:0.4rem">
        <button class="outline btn-xs" data-edit="${esc(c.name)}">Edit</button>
        <button class="outline btn-xs" data-toggle="${esc(c.name)}" data-enabled="${enabled}">${enabled ? 'Disable' : 'Enable'}</button>
        <button class="secondary outline btn-xs" data-delete="${esc(c.name)}">Delete</button>
      </div></td></tr>`
  }).join('')
  el.innerHTML = `<figure><table><thead><tr>
    <th class="sortable${sCls('name')}" data-sort="name">Name<span class="sort-ind">${sInd('name')}</span></th>
    <th class="sortable${sCls('')}" data-sort="">Schedules</th>
    <th class="sortable${sCls('work')}" data-sort="work">Work Ref<span class="sort-ind">${sInd('work')}</span></th>
    <th class="sortable${sCls('enabled')}" data-sort="enabled">Status<span class="sort-ind">${sInd('enabled')}</span></th>
    <th>Next run</th><th></th>
  </tr></thead><tbody>${rows}</tbody></table></figure>
  <div id="crons-pager">${pagerHTML(page, pages, total)}</div>`
  bindListButtons(el, ns)
  el.querySelectorAll('th[data-sort]').forEach(th => th.addEventListener('click', () => {
    const k = th.dataset.sort; if (!k) return
    if (cronState.sortKey === k) cronState.sortDir = cronState.sortDir === 'asc' ? 'desc' : 'asc'
    else { cronState.sortKey = k; cronState.sortDir = 'asc' }
    cronState.page = 1; drawCrons(ns)
  }))
  el.querySelectorAll('[data-page]').forEach(btn => btn.addEventListener('click', () => { cronState.page = +btn.dataset.page; drawCrons(ns) }))
}

function bindListButtons(el, ns) {
  el.querySelectorAll('[data-edit]').forEach(btn => btn.addEventListener('click', () => {
    const cron = allCrons.find(c => c.name === btn.dataset.edit)
    if (cron) openCronModal(ns, cron)
  }))
  el.querySelectorAll('[data-toggle]').forEach(btn => btn.addEventListener('click', async () => {
    const cron = allCrons.find(c => c.name === btn.dataset.toggle)
    if (!cron) return
    const updated = { ...cron, spec: { ...cron.spec, enabled: btn.dataset.enabled !== 'true' } }
    try { await registerCron(updated); toastSuccess('Cron updated.'); await fetchAndDraw(ns) }
    catch (e) { toastError(e.message) }
  }))
  el.querySelectorAll('[data-delete]').forEach(btn => btn.addEventListener('click', async () => {
    if (!confirm(`Delete cron "${btn.dataset.delete}"?`)) return
    try { await unregisterCron(btn.dataset.delete); toastSuccess('Cron deleted.'); await fetchAndDraw(ns) }
    catch (e) { toastError(e.message) }
  }))
}

async function openCronModal(ns, existing) {
  const isEdit = !!existing
  openModal(isEdit ? `Edit: ${existing.name}` : 'New Cron', buildFormHtml(ns, existing))
  const params = existing?.spec?.params ?? []
  const paramsObj = Object.fromEntries(params.map(p => [p.name, p.value ?? '']))
  renderKvEditor('cron-params', paramsObj, {})
  document.getElementById('cron-cancel').addEventListener('click', closeModal)
  document.getElementById('form-cron').addEventListener('submit', e => handleSubmit(e, ns, existing))
}

function buildFormHtml(ns, existing) {
  const e = existing; const s = e?.spec ?? {}
  const scheds = (s.schedules ?? []).join(', ')
  const concOpts = ['allow','skip','replace','wait']
  const concSel = concOpts.map(o => `<option value="${o}" ${(s.concurrency?.same_cron ?? 'skip') === o ? 'selected' : ''}>${o}</option>`).join('')
  return `<form id="form-cron">
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.75rem">
      <label>Name<input type="text" id="cron-name" value="${esc(e?.name ?? '')}" ${e ? 'readonly' : 'required'} placeholder="daily-etl" /></label>
      <label>Schedule(s) <small class="muted">(comma-separated)</small>
        <input type="text" id="cron-schedules" required value="${esc(scheds)}" placeholder="0 1 * * *" /></label>
    </div>
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.75rem">
      <label>Work Ref <small class="muted">(namespace/name:version)</small>
        <input type="text" id="cron-work-ref" required value="${esc(s.work_ref ?? '')}" placeholder="${ns}/my-work:1" /></label>
      <label>Timezone<input type="text" id="cron-tz" value="${esc(s.timezone ?? 'UTC')}" /></label>
    </div>
    <fieldset><legend>Parameters</legend><div id="cron-params"></div></fieldset>
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.75rem">
      <label>Concurrency (same cron)<select id="cron-concurrency">${concSel}</select></label>
      <label style="display:flex;align-items:center;gap:0.5rem;padding-top:1.5rem;cursor:pointer">
        <input type="checkbox" id="cron-enabled" ${s.enabled !== false ? 'checked' : ''} role="switch" /> Enabled</label>
    </div>
    <div style="display:flex;gap:0.5rem;justify-content:flex-end;margin-top:1rem">
      <button type="button" class="secondary outline" id="cron-cancel">Cancel</button>
      <button type="submit">${e ? 'Save' : 'Create'}</button>
    </div>
  </form>`
}

async function handleSubmit(evt, ns, existing) {
  evt.preventDefault()
  const submit = evt.target.querySelector('[type=submit]')
  submit.setAttribute('aria-busy', 'true'); submit.disabled = true
  try {
    const schedules = document.getElementById('cron-schedules').value.split(',').map(s => s.trim()).filter(Boolean)
    const params    = readKv('cron-params').filter(p => p.k).map(p => ({ name: p.k, value: p.v }))
    const body = {
      kind: 'workaholic/Cron:1.0',
      name: document.getElementById('cron-name').value.trim(),
      version: existing?.version ?? '1.0.0',
      metadata: existing?.metadata ?? { namespace: ns },
      spec: {
        enabled:   document.getElementById('cron-enabled').checked,
        work_ref:  document.getElementById('cron-work-ref').value.trim(),
        schedules, params,
        timezone:     document.getElementById('cron-tz').value.trim() || 'UTC',
        concurrency:  { same_cron: document.getElementById('cron-concurrency').value },
      },
    }
    await registerCron(body)
    toastSuccess(existing ? 'Cron updated.' : 'Cron created.')
    closeModal()
    await fetchAndDraw(ns)
  } catch (err) {
    toastError(`Failed to save: ${err.message}`)
  } finally {
    submit.removeAttribute('aria-busy'); submit.disabled = false
  }
}
