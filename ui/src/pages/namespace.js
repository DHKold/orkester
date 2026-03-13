import { listTasks, listWorks } from '../api.js'
import { esc, setApp, nsHeader } from '../utils.js'
import { toastError } from '../components/toast.js'
import { openModal } from '../components/modal.js'
import { renderDag } from '../components/dag.js'

let worksCache = {}

export async function renderNamespace({ ns }) {
  setApp(`
    ${nsHeader(ns, 'Catalog')}
    <p aria-busy="true">Loading catalog…</p>
  `)

  try {
    const [tasksData, worksData] = await Promise.all([
      listTasks(ns), listWorks(ns)
    ])

    const tasks = tasksData.tasks ?? []
    const works = worksData.works ?? []

    worksCache = {}
    works.forEach(w => { worksCache[`${w.name}@${w.version}`] = w })

    const taskRows = tasks.map(t => `
      <tr>
        <td><strong>${esc(t.name)}</strong></td>
        <td><span class="tag">${esc(t.version)}</span></td>
        <td><code class="muted">${esc(t.spec?.executor ?? '—')}</code></td>
        <td class="muted">${esc(t.metadata?.description || '—')}</td>
        <td>${t.spec?.retries ? `${t.spec.retries}×` : '—'}</td>
        <td>${t.spec?.timeout_seconds ? `${t.spec.timeout_seconds}s` : '—'}</td>
      </tr>
    `).join('')

    const workRows = works.map(w => {
      const stepCount = w.spec?.steps?.length ?? 0
      const nsEnc = encodeURIComponent(ns)
      const key = esc(`${w.name}@${w.version}`)
      return `
        <tr>
          <td>
            <button class="plain-link work-detail-btn" data-work-key="${key}">
              <strong>${esc(w.name)}</strong>
            </button>
          </td>
          <td><span class="tag">${esc(w.version)}</span></td>
          <td>${stepCount} step${stepCount !== 1 ? 's' : ''}</td>
          <td class="muted">${esc(w.metadata?.description || '—')}</td>
          <td>
            <a href="#/namespaces/${nsEnc}/workflows" role="button" class="outline btn-xs"
               data-work-name="${esc(w.name)}" data-work-version="${esc(w.version)}">
              ▶ Run
            </a>
          </td>
        </tr>
      `
    }).join('')

    setApp(`
      ${nsHeader(ns, 'Catalog')}

      <section>
        <div class="row-between" style="margin-bottom:0.75rem">
          <h3 style="margin:0">Works <span class="muted" style="font-size:0.85rem">(${works.length})</span></h3>
        </div>
        ${works.length === 0
          ? '<p class="empty-state">No Works defined in this namespace.</p>'
          : `<figure><table>
              <thead><tr><th>Name</th><th>Version</th><th>Steps</th><th>Description</th><th></th></tr></thead>
              <tbody>${workRows}</tbody>
            </table></figure>`
        }
      </section>

      <section>
        <div class="row-between" style="margin-bottom:0.75rem">
          <h3 style="margin:0">Tasks <span class="muted" style="font-size:0.85rem">(${tasks.length})</span></h3>
        </div>
        ${tasks.length === 0
          ? '<p class="empty-state">No Tasks defined in this namespace.</p>'
          : `<figure><table>
              <thead><tr><th>Name</th><th>Version</th><th>Executor</th><th>Description</th><th>Retries</th><th>Timeout</th></tr></thead>
              <tbody>${taskRows}</tbody>
            </table></figure>`
        }
      </section>
    `)

    // Work detail click
    document.querySelectorAll('.work-detail-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        const work = worksCache[btn.dataset.workKey]
        if (work) showWorkDetail(work)
      })
    })

    // "Run" buttons navigate to workflows list with pre-selection via query
    document.querySelectorAll('[data-work-name]').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.preventDefault()
        const wn  = btn.dataset.workName
        const wv  = btn.dataset.workVersion
        const nsE = encodeURIComponent(ns)
        window.location.hash = `#/namespaces/${nsE}/workflows?new=${encodeURIComponent(wn)}&ver=${encodeURIComponent(wv)}`
      })
    })

  } catch (e) {
    toastError(`Failed to load catalog: ${e.message}`)
  }
}

function showWorkDetail(work) {
  const steps  = work.spec?.steps ?? []
  const inputs = work.spec?.inputs ?? {}

  const inputRows = Object.entries(inputs).map(([k, v]) =>
    `<tr>
      <td><code>${esc(k)}</code></td>
      <td class="muted">${esc(typeof v === 'string' ? v : (v?.description ?? '—'))}</td>
    </tr>`
  ).join('')

  const html = `
    <div class="row" style="margin-bottom:0.75rem">
      <span class="tag">${esc(work.version ?? '')}</span>
      <span class="muted">${steps.length} step${steps.length !== 1 ? 's' : ''}</span>
    </div>
    ${work.metadata?.description ? `<p>${esc(work.metadata.description)}</p>` : ''}
    ${Object.keys(inputs).length > 0 ? `
      <details style="margin-bottom:1rem">
        <summary><strong>Inputs</strong></summary>
        <figure>
          <table>
            <thead><tr><th>Name</th><th>Description</th></tr></thead>
            <tbody>${inputRows}</tbody>
          </table>
        </figure>
      </details>
    ` : ''}
    <div id="work-detail-dag" style="width:100%;height:320px;border:1px solid var(--pico-muted-border-color,#e2e8f0);border-radius:0.5rem;background:#f8fafc"></div>
  `

  openModal(work.name, html)
  // showModal() is synchronous — container is visible immediately
  const dagEl = document.getElementById('work-detail-dag')
  if (dagEl) renderDag(dagEl, work, {}, null)
}
