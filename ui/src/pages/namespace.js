import { listTasks, listWorks } from '../api.js'
import { esc, setApp, breadcrumb, applyFilter, applySort, paginate, pagerHTML } from '../utils.js'
import { toastError } from '../components/toast.js'
import { openModal } from '../components/modal.js'
import { renderDag } from '../components/dag.js'

let worksCache = {}

export async function renderNamespace({ ns }) {
  setApp(`
    ${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns}])}
    <p aria-busy="true">Loading catalog…</p>
  `)

  try {
    const [tasksData, worksData] = await Promise.all([listTasks(ns), listWorks(ns)])
    const allWorks = worksData.works ?? []
    const allTasks = tasksData.tasks ?? []

    worksCache = {}
    allWorks.forEach(w => { worksCache[`${w.name}@${w.version}`] = w })

    const nsEnc = encodeURIComponent(ns)

    setApp(`
      ${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns}])}

      <section>
        <div class="row-between" style="margin-bottom:0.75rem">
          <h3 style="margin:0">Works <span class="muted" id="works-count" style="font-size:0.85rem"></span></h3>
          <input type="search" id="works-filter" placeholder="Filter by name…" class="list-filter" />
        </div>
        ${allWorks.length === 0
          ? '<p class="empty-state">No Works defined in this namespace.</p>'
          : `<figure>
              <table id="works-table">
                <thead><tr>
                  <th class="sortable" data-sort="name">Name<span class="sort-ind"></span></th>
                  <th class="sortable" data-sort="version">Version<span class="sort-ind"></span></th>
                  <th>Steps</th><th>Description</th><th></th>
                </tr></thead>
                <tbody></tbody>
              </table>
            </figure>
            <div id="works-pager"></div>`
        }
      </section>

      <section>
        <div class="row-between" style="margin-bottom:0.75rem">
          <h3 style="margin:0">Tasks <span class="muted" id="tasks-count" style="font-size:0.85rem"></span></h3>
          <input type="search" id="tasks-filter" placeholder="Filter by name…" class="list-filter" />
        </div>
        ${allTasks.length === 0
          ? '<p class="empty-state">No Tasks defined in this namespace.</p>'
          : `<figure>
              <table id="tasks-table">
                <thead><tr>
                  <th class="sortable" data-sort="name">Name<span class="sort-ind"></span></th>
                  <th class="sortable" data-sort="version">Version<span class="sort-ind"></span></th>
                  <th>Executor</th><th>Description</th><th>Inputs</th><th>Outputs</th>
                </tr></thead>
                <tbody></tbody>
              </table>
            </figure>
            <div id="tasks-pager"></div>`
        }
      </section>
    `)

    // ── Works state & draw ────────────────────────────────────────────────────
    const ws = { q: '', sortKey: 'name', sortDir: 'asc', page: 1 }
    const WORK_SORT = { name: w => w.name, version: w => w.version }

    const drawWorks = () => {
      const table = document.getElementById('works-table')
      if (!table) return
      const filtered = applyFilter(allWorks, ws.q, w => w.name, w => w.version)
      const sorted   = applySort(filtered, WORK_SORT[ws.sortKey], ws.sortDir)
      const { slice, page, pages, total } = paginate(sorted, ws.page)
      ws.page = page

      const countEl = document.getElementById('works-count')
      if (countEl) countEl.textContent = total < allWorks.length ? `(${total} of ${allWorks.length})` : `(${total})`

      table.querySelector('tbody').innerHTML = slice.map(w => {
        const stepCount = w.spec?.steps?.length ?? 0
        const key = esc(`${w.name}@${w.version}`)
        return `<tr>
          <td><button class="plain-link work-detail-btn" data-work-key="${key}"><strong>${esc(w.name)}</strong></button></td>
          <td><span class="tag">${esc(w.version)}</span></td>
          <td>${stepCount} step${stepCount !== 1 ? 's' : ''}</td>
          <td class="muted">${esc(w.metadata?.description || '—')}</td>
          <td><a href="#/namespaces/${nsEnc}/workflows" role="button" class="outline btn-xs"
            data-work-name="${esc(w.name)}" data-work-version="${esc(w.version)}">▶ Run</a></td>
        </tr>`
      }).join('')

      table.querySelectorAll('th[data-sort]').forEach(th => {
        const k = th.dataset.sort
        th.querySelector('.sort-ind').textContent = k === ws.sortKey ? (ws.sortDir === 'asc' ? ' ▲' : ' ▼') : ' ⇅'
        th.classList.toggle('sort-active', k === ws.sortKey)
      })

      const pagerEl = document.getElementById('works-pager')
      if (pagerEl) {
        pagerEl.innerHTML = pagerHTML(page, pages, total)
        pagerEl.querySelectorAll('[data-page]').forEach(btn =>
          btn.addEventListener('click', () => { ws.page = +btn.dataset.page; drawWorks() })
        )
      }

      table.querySelectorAll('.work-detail-btn').forEach(btn =>
        btn.addEventListener('click', () => { const work = worksCache[btn.dataset.workKey]; if (work) showWorkDetail(work) })
      )
      table.querySelectorAll('[data-work-name]').forEach(btn =>
        btn.addEventListener('click', (e) => {
          e.preventDefault()
          window.location.hash = `#/namespaces/${nsEnc}/workflows?new=${encodeURIComponent(btn.dataset.workName)}&ver=${encodeURIComponent(btn.dataset.workVersion)}`
        })
      )
    }

    if (allWorks.length > 0) {
      document.getElementById('works-filter').addEventListener('input', (e) => {
        ws.q = e.target.value; ws.page = 1; drawWorks()
      })
      document.getElementById('works-table').querySelectorAll('th[data-sort]').forEach(th =>
        th.addEventListener('click', () => {
          const k = th.dataset.sort
          if (ws.sortKey === k) ws.sortDir = ws.sortDir === 'asc' ? 'desc' : 'asc'
          else { ws.sortKey = k; ws.sortDir = 'asc' }
          ws.page = 1; drawWorks()
        })
      )
      drawWorks()
    }

    // ── Tasks state & draw ───────────────────────────────────────────────────
    const ts = { q: '', sortKey: 'name', sortDir: 'asc', page: 1 }
    const TASK_SORT = { name: t => t.name, version: t => t.version }

    const drawTasks = () => {
      const table = document.getElementById('tasks-table')
      if (!table) return
      const filtered = applyFilter(allTasks, ts.q, t => t.name, t => t.version)
      const sorted   = applySort(filtered, TASK_SORT[ts.sortKey], ts.sortDir)
      const { slice, page, pages, total } = paginate(sorted, ts.page)
      ts.page = page

      const countEl = document.getElementById('tasks-count')
      if (countEl) countEl.textContent = total < allTasks.length ? `(${total} of ${allTasks.length})` : `(${total})`

      table.querySelector('tbody').innerHTML = slice.map(t => `<tr>
        <td><strong>${esc(t.name)}</strong></td>
        <td><span class="tag">${esc(t.version)}</span></td>
        <td><code class="muted">${esc(t.spec?.execution?.kind ?? '—')}</code></td>
        <td class="muted">${esc(t.metadata?.description || '—')}</td>
        <td>${t.spec?.inputs?.length ?? 0}</td>
        <td>${t.spec?.outputs?.length ?? 0}</td>
      </tr>`).join('')

      table.querySelectorAll('th[data-sort]').forEach(th => {
        const k = th.dataset.sort
        th.querySelector('.sort-ind').textContent = k === ts.sortKey ? (ts.sortDir === 'asc' ? ' ▲' : ' ▼') : ' ⇅'
        th.classList.toggle('sort-active', k === ts.sortKey)
      })

      const pagerEl = document.getElementById('tasks-pager')
      if (pagerEl) {
        pagerEl.innerHTML = pagerHTML(page, pages, total)
        pagerEl.querySelectorAll('[data-page]').forEach(btn =>
          btn.addEventListener('click', () => { ts.page = +btn.dataset.page; drawTasks() })
        )
      }
    }

    if (allTasks.length > 0) {
      document.getElementById('tasks-filter').addEventListener('input', (e) => {
        ts.q = e.target.value; ts.page = 1; drawTasks()
      })
      document.getElementById('tasks-table').querySelectorAll('th[data-sort]').forEach(th =>
        th.addEventListener('click', () => {
          const k = th.dataset.sort
          if (ts.sortKey === k) ts.sortDir = ts.sortDir === 'asc' ? 'desc' : 'asc'
          else { ts.sortKey = k; ts.sortDir = 'asc' }
          ts.page = 1; drawTasks()
        })
      )
      drawTasks()
    }

  } catch (e) {
    toastError(`Failed to load catalog: ${e.message}`)
  }
}

function showWorkDetail(work) {
  const steps   = work.spec?.steps  ?? []
  const inputs  = Array.isArray(work.spec?.inputs)  ? work.spec.inputs  : []
  const outputs = Array.isArray(work.spec?.outputs) ? work.spec.outputs : []
  const tags    = work.metadata?.tags   ?? []
  const owner   = work.metadata?.owner  ?? ''

  const inputRows = inputs.map(inp =>
    `<tr>
      <td><code>${esc(inp.name)}</code>${inp.required === false ? ' <span class="tag" style="font-size:0.75rem">optional</span>' : ''}</td>
      <td class="muted">${esc(inp.description ?? '—')}</td>
      <td><code>${esc(inp.type ?? inp.input_type ?? inp.inputType ?? '—')}</code></td>
      <td>${inp.default != null ? `<code>${esc(JSON.stringify(inp.default))}</code>` : '<span class="muted">—</span>'}</td>
    </tr>`
  ).join('')

  const outputRows = outputs.map(out =>
    `<tr>
      <td><code>${esc(out.name)}</code></td>
      <td class="muted">${esc(out.description ?? '—')}</td>
      <td><code>${esc(out.type ?? out.output_type ?? out.outputType ?? '—')}</code></td>
    </tr>`
  ).join('')

  const stepCards = steps.map(s => {
    const deps     = s.depends_on ?? s.dependsOn ?? []
    const inMap    = s.input_mapping  ?? s.inputMapping  ?? []
    const outMap   = s.output_mapping ?? s.outputMapping ?? []
    const taskRef   = s.task_ref ?? s.taskRef ?? '—'
    return `
      <div style="border-left:3px solid var(--pico-primary,#3b82f6);padding:0.4rem 0.75rem;margin-bottom:0.5rem;background:var(--pico-card-background,#fff);border-radius:0 4px 4px 0">
        <div class="row" style="gap:0.5rem;flex-wrap:wrap;align-items:center">
          <strong>${esc(s.name)}</strong>
          <code class="muted" style="font-size:0.8rem">${esc(taskRef)}</code>
          ${deps.length ? `<span class="muted" style="font-size:0.78rem">← ${deps.map(d => `<code>${esc(d)}</code>`).join(', ')}</span>` : ''}
        </div>
        ${s.description ? `<p class="muted" style="margin:0.2rem 0 0;font-size:0.82rem">${esc(s.description)}</p>` : ''}
        ${inMap.length > 0 ? `
          <div style="font-size:0.78rem;margin-top:0.25rem">
            <span class="muted">inputs:</span>
            ${inMap.map(m => `<code>${esc(m.name)}</code>`).join(', ')}
          </div>` : ''}
        ${outMap.length > 0 ? `
          <div style="font-size:0.78rem">
            <span class="muted">outputs:</span>
            ${outMap.map(m => `<code>${esc(m.name)}</code>`).join(', ')}
          </div>` : ''}
      </div>`
  }).join('')

  const html = `
    <div class="row" style="margin-bottom:0.75rem;flex-wrap:wrap;gap:0.4rem;align-items:center">
      <span class="tag">${esc(work.version ?? '')}</span>
      <span class="muted" style="font-size:0.85rem">${steps.length} step${steps.length !== 1 ? 's' : ''}</span>
      ${tags.map(t => `<span class="tag" style="background:#dbeafe;color:#1e40af">${esc(t)}</span>`).join('')}
      ${owner ? `<span class="muted" style="font-size:0.82rem">owner: <strong>${esc(owner)}</strong></span>` : ''}
    </div>
    ${work.metadata?.description ? `<p style="margin-bottom:0.75rem">${esc(work.metadata.description)}</p>` : ''}

    ${inputs.length > 0 ? `
      <details open style="margin-bottom:0.75rem">
        <summary><strong>Inputs</strong> <span class="muted">(${inputs.length})</span></summary>
        <figure style="margin-top:0.5rem">
          <table>
            <thead><tr><th>Name</th><th>Description</th><th>Type</th><th>Default</th></tr></thead>
            <tbody>${inputRows}</tbody>
          </table>
        </figure>
      </details>
    ` : ''}

    ${outputs.length > 0 ? `
      <details style="margin-bottom:0.75rem">
        <summary><strong>Outputs</strong> <span class="muted">(${outputs.length})</span></summary>
        <figure style="margin-top:0.5rem">
          <table>
            <thead><tr><th>Name</th><th>Description</th><th>Type</th></tr></thead>
            <tbody>${outputRows}</tbody>
          </table>
        </figure>
      </details>
    ` : ''}

    ${steps.length > 0 ? `
      <details open style="margin-bottom:0.75rem">
        <summary><strong>Steps</strong> <span class="muted">(${steps.length})</span></summary>
        <div style="margin-top:0.5rem">${stepCards}</div>
      </details>
    ` : ''}

    <details open style="margin-bottom:0.5rem">
      <summary><strong>Execution Graph</strong></summary>
      <div id="work-detail-dag" style="width:100%;height:280px;border:1px solid var(--pico-muted-border-color,#e2e8f0);border-radius:0.5rem;background:#f8fafc;margin-top:0.5rem"></div>
    </details>
  `

  openModal(work.name, html)
  const dagEl = document.getElementById('work-detail-dag')
  if (dagEl) renderDag(dagEl, work, {}, null)
}
