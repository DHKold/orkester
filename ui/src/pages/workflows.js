import { listWorkflows, deleteWorkflow, createWorkflow, listWorks, getWork } from '../api.js'
import { esc, fmtDateShort, fmtDuration, badge, setApp, nsHeader, renderKvEditor, readKv, kvToObject } from '../utils.js'
import { toastError, toastSuccess } from '../components/toast.js'
import { openModal, closeModal } from '../components/modal.js'
import { navigate } from '../router.js'

const TERMINAL = new Set(['succeeded', 'failed', 'cancelled'])
const STATUS_ORDER = ['running', 'waiting', 'paused', 'failed', 'succeeded', 'cancelled']

export async function renderWorkflows({ ns, query = {} }) {
  setApp(`
    ${nsHeader(ns, 'Workflows')}
    <p aria-busy="true">Loading workflows…</p>
  `)

  // ?new=workName&ver=workVersion pre-selection from Catalog "Run" button
  const preWork = query['new'] ?? null
  const preVer  = query['ver'] ?? null

  await load(ns, preWork, preVer)
}

async function load(ns, preWork = null, preVer = null) {
  try {
    const data      = await listWorkflows(ns)
    const workflows = (data.workflows ?? [])
      .sort((a, b) => new Date(b.created_at) - new Date(a.created_at))

    const nsEnc     = encodeURIComponent(ns)
    const filterTabs = STATUS_ORDER.concat(['all'])
    const activeFilt = 'all' // could be persisted in hash later

    const rows = workflows.map(wf => {
      const id     = esc(wf.id)
      const wfEnc  = encodeURIComponent(wf.id)
      const status = wf.status ?? 'waiting'
      const dur    = TERMINAL.has(status)
        ? fmtDuration(wf.started_at, wf.finished_at)
        : wf.started_at ? fmtDuration(wf.started_at) + ' ⏱' : '—'

      const metrics  = wf.metrics ?? {}
      const progress = metrics.steps_total > 0
        ? `${metrics.steps_succeeded}/${metrics.steps_total}`
        : '—'

      return `
        <tr data-status="${esc(status)}">
          <td>
            <a href="#/namespaces/${nsEnc}/workflows/${wfEnc}">
              <code style="font-size:0.78em">${id.substring(0, 8)}…</code>
            </a>
          </td>
          <td>
            <strong>${esc(wf.work_name)}</strong>
            <span class="muted"> @ ${esc(wf.work_version)}</span>
          </td>
          <td>${badge(status)}</td>
          <td class="muted">${fmtDateShort(wf.created_at)}</td>
          <td class="muted">${dur}</td>
          <td class="muted">${progress}</td>
          <td>
            <div style="display:flex;gap:0.4rem">
              <a href="#/namespaces/${nsEnc}/workflows/${wfEnc}" role="button" class="outline btn-xs">View</a>
              ${!TERMINAL.has(status)
                ? `<button class="secondary outline btn-xs" data-cancel="${esc(wf.id)}">Cancel</button>`
                : `<button class="secondary outline btn-xs" data-delete="${esc(wf.id)}">Delete</button>`
              }
            </div>
          </td>
        </tr>
      `
    }).join('')

    setApp(`
      ${nsHeader(ns, 'Workflows')}
      <div class="row-between" style="margin-bottom:1rem">
        <h3 style="margin:0">Workflows
          <span class="muted" style="font-size:0.85rem">(${workflows.length})</span>
        </h3>
        <button id="btn-new-workflow">+ New Workflow</button>
      </div>

      ${workflows.length === 0
        ? '<div class="empty-state"><p>No workflows yet. Create one to get started.</p></div>'
        : `<figure><table>
            <thead>
              <tr>
                <th>ID</th><th>Work</th><th>Status</th>
                <th>Created</th><th>Duration</th><th>Steps</th><th></th>
              </tr>
            </thead>
            <tbody>${rows}</tbody>
          </table></figure>`
      }
    `)

    document.getElementById('btn-new-workflow')
      .addEventListener('click', () => openCreateModal(ns, preWork, preVer))

    document.querySelectorAll('[data-delete]').forEach(btn => {
      btn.addEventListener('click', async () => {
        if (!confirm(`Delete workflow ${btn.dataset.delete}?`)) return
        try {
          await deleteWorkflow(ns, btn.dataset.delete)
          toastSuccess('Workflow deleted.')
          await load(ns)
        } catch (e) { toastError(e.message) }
      })
    })

    document.querySelectorAll('[data-cancel]').forEach(btn => {
      btn.addEventListener('click', async () => {
        if (!confirm(`Cancel workflow ${btn.dataset.cancel}?`)) return
        try {
          const { updateWorkflow } = await import('../api.js')
          await updateWorkflow(ns, btn.dataset.cancel, { status: 'cancelled' })
          toastSuccess('Workflow cancelled.')
          await load(ns)
        } catch (e) { toastError(e.message) }
      })
    })

    // If pre-selected work from Catalog, open modal immediately
    if (preWork) openCreateModal(ns, preWork, preVer)

  } catch (e) {
    toastError(`Failed to load workflows: ${e.message}`)
  }
}

// ── Create Workflow Modal ─────────────────────────────────────────────────────

async function openCreateModal(ns, preWorkName = null, preWorkVer = null) {
  openModal('New Workflow', '<p aria-busy="true">Loading works…</p>')

  try {
    const data  = await listWorks(ns)
    const works = data.works ?? []

    const options = works.map(w =>
      `<option value="${esc(w.name)}|${esc(w.version)}"
        ${w.name === preWorkName && w.version === preWorkVer ? 'selected' : ''}>
        ${esc(w.name)} @ ${esc(w.version)}
      </option>`
    ).join('')

    const html = `
      <form id="form-create-wf">
        <label>Work
          <select id="wf-work" required>
            <option value="">Select a Work…</option>
            ${options}
          </select>
        </label>

        <details id="wf-schedule-details">
          <summary>Schedule (optional)</summary>
          <label>Start at
            <input type="datetime-local" id="wf-start-dt" />
            <small>Leave blank to start immediately.</small>
          </label>
        </details>

        <fieldset>
          <legend>Parameters</legend>
          <div id="wf-context"></div>
        </fieldset>

        <div style="display:flex;gap:0.5rem;justify-content:flex-end;margin-top:1rem">
          <button type="button" class="secondary outline" id="wf-cancel-btn">Cancel</button>
          <button type="submit" id="wf-submit-btn">Create Workflow</button>
        </div>
      </form>
    `

    const { setModalBody } = await import('./modal.js').catch(() => import('../components/modal.js'))
    document.getElementById('modal-body').innerHTML = html

    // Pre-populate context if a work was pre-selected
    const workSelect = document.getElementById('wf-work')
    const loadContext = async (val) => {
      if (!val) { renderKvEditor('wf-context', {}, {}); return }
      const [wn, wv] = val.split('|')
      try {
        const w = await getWork(ns, wn, wv)
        renderKvEditor('wf-context', {}, w.spec?.inputs ?? {})
      } catch (_) { renderKvEditor('wf-context', {}, {}) }
    }

    workSelect.addEventListener('change', (e) => loadContext(e.target.value))
    await loadContext(workSelect.value)

    document.getElementById('wf-cancel-btn').addEventListener('click', closeModal)

    document.getElementById('form-create-wf').addEventListener('submit', async (e) => {
      e.preventDefault()
      const [wn, wv] = (workSelect.value || '').split('|')
      if (!wn) { toastError('Please select a Work.'); return }

      const submit = document.getElementById('wf-submit-btn')
      submit.setAttribute('aria-busy', 'true')
      submit.disabled = true

      try {
        const context = kvToObject(readKv('wf-context'))
        const startDt = document.getElementById('wf-start-dt').value
        const body = {
          work_name:    wn,
          work_version: wv,
          work_context: context,
          ...(startDt ? { schedule: { start_datetime: new Date(startDt).toISOString() } } : {}),
        }
        await createWorkflow(ns, body)
        closeModal()
        toastSuccess('Workflow created.')
        await load(ns)
      } catch (err) {
        toastError(`Failed to create workflow: ${err.message}`)
      } finally {
        submit.removeAttribute('aria-busy')
        submit.disabled = false
      }
    })

  } catch (e) {
    document.getElementById('modal-body').innerHTML =
      `<p class="muted">Failed to load works: ${esc(e.message)}</p>`
    toastError(e.message)
  }
}
