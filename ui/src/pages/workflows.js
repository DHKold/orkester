import { listWorkRuns, cancelWorkRun, triggerWork, listWorks } from '../api.js'
import { esc, fmtDateShort, fmtDuration, badge, setApp, breadcrumb, renderKvEditor, readKv, kvToObject, applyFilter, applySort, paginate, pagerHTML } from '../utils.js'
import { toastError, toastSuccess } from '../components/toast.js'
import { openModal, closeModal } from '../components/modal.js'
import { navigate, setCleanup } from '../router.js'

const TERMINAL = new Set(['succeeded', 'failed', 'cancelled'])
const REFRESH_INTERVAL = 30

let wfItems = []
let wfState = { q: '', sortKey: 'created_at', sortDir: 'desc', page: 1 }

export async function renderWorkflows({ ns, query = {} }) {
  const preWork = query['new'] ?? null

  wfState = { q: '', sortKey: 'created_at', sortDir: 'desc', page: 1 }
  wfItems = []

  setApp(`
    ${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns,href:`#/namespaces/${encodeURIComponent(ns)}`},{label:'Workflows'}])}
    <div class="row-between" style="margin-bottom:1rem">
      <div class="row" style="gap:0.75rem;align-items:center">
        <h3 style="margin:0">Workflows <span class="muted" id="wf-count" style="font-size:0.85rem"></span></h3>
        <input type="search" id="wf-filter" placeholder="Filter…" class="list-filter" />
      </div>
      <div class="row" style="gap:0.75rem;align-items:center">
        <span id="wf-refresh-status" class="muted" style="font-size:0.82rem"></span>
        <label class="toggle-switch" title="Auto-refresh">
          <input type="checkbox" id="chk-refresh-toggle" checked />
          <span class="toggle-track"><span class="toggle-thumb"></span></span>
        </label>
        <button id="btn-refresh-now" class="outline btn-xs" title="Refresh now" style="font-size:1.1rem;line-height:1;padding:0.18rem 0.5rem">⟳</button>
        <button id="btn-new-workflow" class="outline btn-sm">New Workflow</button>
      </div>
    </div>
    <div id="wf-list"><p aria-busy="true">Loading workflows…</p></div>
  `)

  let countdown = REFRESH_INTERVAL
  let paused    = false
  const updateStatus = () => {
    const el = document.getElementById('wf-refresh-status')
    if (el) el.textContent = paused ? 'auto-refresh off' : `refresh in ${countdown}s`
  }
  const doRefresh = async () => {
    countdown = REFRESH_INTERVAL; updateStatus(); await loadList(ns); updateStatus()
  }
  const timer = setInterval(() => {
    if (paused) return
    countdown--; updateStatus()
    if (countdown <= 0) doRefresh()
  }, 1000)
  setCleanup(() => clearInterval(timer))
  updateStatus()

  document.getElementById('chk-refresh-toggle').addEventListener('change', (e) => {
    paused = !e.target.checked
    if (!paused) { countdown = REFRESH_INTERVAL }
    updateStatus()
  })
  document.getElementById('btn-refresh-now').addEventListener('click', () => doRefresh())
  document.getElementById('btn-new-workflow').addEventListener('click', () => openTriggerModal(ns, preWork))
  document.getElementById('wf-filter').addEventListener('input', (e) => {
    wfState.q = e.target.value; wfState.page = 1; renderWfList(ns)
  })

  await loadList(ns, preWork, /* openPreModal= */ true)
}

async function loadList(ns, preWork = null, openPreModal = false) {
  try {
    const data = await listWorkRuns()
    // Filter by namespace: spec.work_ref starts with `{ns}/` or metadata shows namespace
    wfItems = (data.work_runs ?? []).filter(wr => {
      const wrNs = wr.metadata?.namespace
      if (wrNs) return wrNs === ns
      return (wr.spec?.work_ref ?? '').startsWith(ns + '/')
    })
    renderWfList(ns)
    if (openPreModal && preWork) openTriggerModal(ns, preWork)
  } catch (e) {
    toastError(`Failed to load workflows: ${e.message}`)
    const el = document.getElementById('wf-list')
    if (el) el.innerHTML = '<div class="empty-state"><p>Failed to load workflows.</p></div>'
  }
}

function workRunState(wr) { return wr.status?.state ?? 'pending' }
function workRunRef(wr) { return wr.spec?.work_ref ?? '—' }
function workRunCreatedAt(wr) { return wr.status?.created_at ?? '' }

function renderWfList(ns) {
  const el = document.getElementById('wf-list')
  if (!el) return

  const SORT_FNS = {
    created_at: wr => workRunCreatedAt(wr),
    work:       wr => workRunRef(wr),
    status:     wr => workRunState(wr),
  }

  const filtered = applyFilter(wfItems, wfState.q, wr => workRunRef(wr), wr => wr.name)
  const sorted   = applySort(filtered, SORT_FNS[wfState.sortKey], wfState.sortDir)
  const { slice, page, pages, total } = paginate(sorted, wfState.page)
  wfState.page = page

  const countEl = document.getElementById('wf-count')
  if (countEl) countEl.textContent = total < wfItems.length ? `(${total} of ${wfItems.length})` : `(${total})`

  if (wfItems.length === 0) {
    el.innerHTML = '<div class="empty-state"><p>No workflows yet. Trigger one to get started.</p></div>'
    return
  }
  if (filtered.length === 0) {
    el.innerHTML = '<div class="empty-state"><p>No workflows match the current filter.</p></div>'
    return
  }

  const nsEnc  = encodeURIComponent(ns)
  const sortInd  = k => k === wfState.sortKey ? (wfState.sortDir === 'asc' ? ' ▲' : ' ▼') : ' ⇅'
  const activeCls = k => k === wfState.sortKey ? ' sort-active' : ''

  const rows = slice.map(wr => {
    const name   = esc(wr.name)
    const enc    = encodeURIComponent(wr.name)
    const status = workRunState(wr)
    const st     = wr.status ?? {}
    const dur    = TERMINAL.has(status)
      ? fmtDuration(st.started_at, st.finished_at)
      : st.started_at ? fmtDuration(st.started_at) + ' ⏱' : '—'
    const summary = st.summary ?? {}
    const progress = summary.total_steps > 0
      ? `${summary.succeeded_steps ?? 0}/${summary.total_steps}`
      : '—'
    return `
      <tr data-status="${esc(status)}">
        <td><a href="#/namespaces/${nsEnc}/workflows/${enc}"><code style="font-size:0.78em">${name.substring(0, 8)}…</code></a></td>
        <td><strong>${esc(workRunRef(wr))}</strong></td>
        <td>${badge(status)}</td>
        <td class="muted">${fmtDateShort(workRunCreatedAt(wr))}</td>
        <td class="muted">${dur}</td>
        <td class="muted">${progress}</td>
        <td><div style="display:flex;gap:0.4rem">
          <a href="#/namespaces/${nsEnc}/workflows/${enc}" role="button" class="outline btn-xs">View</a>
          ${!TERMINAL.has(status)
            ? `<button class="secondary outline btn-xs" data-cancel="${esc(wr.name)}">Cancel</button>`
            : ''
          }
        </div></td>
      </tr>`
  }).join('')

  el.innerHTML = `
    <figure><table>
      <thead><tr>
        <th>ID</th>
        <th class="sortable${activeCls('work')}" data-sort="work">Work Ref<span class="sort-ind">${sortInd('work')}</span></th>
        <th class="sortable${activeCls('status')}" data-sort="status">Status<span class="sort-ind">${sortInd('status')}</span></th>
        <th class="sortable${activeCls('created_at')}" data-sort="created_at">Created<span class="sort-ind">${sortInd('created_at')}</span></th>
        <th>Duration</th><th>Steps</th><th></th>
      </tr></thead>
      <tbody>${rows}</tbody>
    </table></figure>
    <div id="wf-pager">${pagerHTML(page, pages, total)}</div>
  `

  el.querySelectorAll('th[data-sort]').forEach(th =>
    th.addEventListener('click', () => {
      const k = th.dataset.sort
      if (wfState.sortKey === k) wfState.sortDir = wfState.sortDir === 'asc' ? 'desc' : 'asc'
      else { wfState.sortKey = k; wfState.sortDir = 'asc' }
      wfState.page = 1; renderWfList(ns)
    })
  )
  el.querySelectorAll('[data-page]').forEach(btn =>
    btn.addEventListener('click', () => { wfState.page = +btn.dataset.page; renderWfList(ns) })
  )
  el.querySelectorAll('[data-cancel]').forEach(btn =>
    btn.addEventListener('click', async () => {
      if (!confirm(`Cancel workflow run ${btn.dataset.cancel}?`)) return
      try {
        await cancelWorkRun(btn.dataset.cancel)
        toastSuccess('Workflow cancelled.')
        await loadList(ns)
      } catch (e) { toastError(e.message) }
    })
  )
}

// ── Trigger Workflow Modal ────────────────────────────────────────────────────

async function openTriggerModal(ns, preWorkRef = null) {
  openModal('Trigger Workflow', '<p aria-busy="true">Loading works…</p>')

  try {
    const data  = await listWorks(ns)
    const works = data.works ?? []

    const options = works.map(w => {
      const ref = `${esc(w.metadata?.namespace ?? ns)}/${esc(w.name)}`
      return `<option value="${ref}" ${ref === preWorkRef ? 'selected' : ''}>${ref}</option>`
    }).join('')

    document.getElementById('modal-body').innerHTML = `
      <form id="form-trigger-wf">
        <label>Work
          <select id="wf-work" required>
            <option value="">Select a Work…</option>
            ${options}
          </select>
        </label>
        <fieldset>
          <legend>Inputs (optional)</legend>
          <div id="wf-inputs"></div>
        </fieldset>
        <div style="display:flex;gap:0.5rem;justify-content:flex-end;margin-top:1rem">
          <button type="button" class="secondary outline" id="wf-cancel-btn">Cancel</button>
          <button type="submit" id="wf-submit-btn">Trigger</button>
        </div>
      </form>
    `

    renderKvEditor('wf-inputs', {})
    document.getElementById('wf-cancel-btn').addEventListener('click', closeModal)
    document.getElementById('form-trigger-wf').addEventListener('submit', async (e) => {
      e.preventDefault()
      const workRef = document.getElementById('wf-work').value
      if (!workRef) { toastError('Please select a Work.'); return }
      const submit = document.getElementById('wf-submit-btn')
      submit.setAttribute('aria-busy', 'true'); submit.disabled = true
      try {
        await triggerWork({ workRef, inputs: kvToObject(readKv('wf-inputs')) })
        closeModal(); toastSuccess('Workflow triggered.'); await loadList(ns)
      } catch (err) {
        toastError(`Failed to trigger: ${err.message}`)
      } finally {
        submit.removeAttribute('aria-busy'); submit.disabled = false
      }
    })
  } catch (e) {
    document.getElementById('modal-body').innerHTML =
      `<p class="muted">Failed to load works: ${esc(e.message)}</p>`
  }
}
