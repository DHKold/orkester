import { listTasks, listWorks } from '../api.js'
import { esc, setApp, breadcrumb, applyFilter, applySort, paginate, pagerHTML } from '../utils.js'
import { toastError } from '../components/toast.js'
import { showWorkDetail, showTaskDetail } from '../components/catalog-detail.js'

let worksCache = {}
let activeTab  = 'works'

export async function renderNamespace({ ns }) {
  activeTab = 'works'
  setApp(`${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns}])}<p aria-busy="true">Loading catalog…</p>`)
  try {
    const [tasksData, worksData] = await Promise.all([listTasks(ns), listWorks(ns)])
    const allWorks = worksData.works ?? []
    const allTasks = tasksData.tasks ?? []
    worksCache = Object.fromEntries(allWorks.map(w => [`${w.name}@${w.version}`, w]))
    const nsEnc = encodeURIComponent(ns)
    setApp(`
      ${breadcrumb([{label:'Namespaces',href:'#/namespaces'},{label:ns}])}
      <div class="tab-bar" style="display:flex;gap:0.5rem;margin-bottom:1.25rem;border-bottom:1px solid var(--pico-muted-border-color,#e2e8f0);padding-bottom:0.5rem">
        <button id="tab-works" class="tab-btn tab-active" data-tab="works">Works <span class="tag">${allWorks.length}</span></button>
        <button id="tab-tasks" class="tab-btn" data-tab="tasks">Tasks <span class="tag">${allTasks.length}</span></button>
      </div>
      <div id="tab-content-works">${worksSection(allWorks)}</div>
      <div id="tab-content-tasks" style="display:none">${tasksSection(allTasks)}</div>
    `)
    bindTabs()
    wireWorks(allWorks, nsEnc)
    wireTasks(allTasks)
  } catch (e) { toastError(`Failed to load catalog: ${e.message}`) }
}

function bindTabs() {
  document.querySelectorAll('.tab-btn').forEach(btn => btn.addEventListener('click', () => {
    activeTab = btn.dataset.tab
    document.querySelectorAll('.tab-btn').forEach(b => b.classList.toggle('tab-active', b.dataset.tab === activeTab))
    document.getElementById('tab-content-works').style.display = activeTab === 'works' ? '' : 'none'
    document.getElementById('tab-content-tasks').style.display = activeTab === 'tasks' ? '' : 'none'
  }))
}

function worksSection(allWorks) {
  if (!allWorks.length) return '<p class="empty-state">No Works in this namespace.</p>'
  return `<div class="row-between" style="margin-bottom:0.75rem">
    <h3 style="margin:0">Works <span class="muted" id="works-count" style="font-size:0.85rem"></span></h3>
    <input type="search" id="works-filter" placeholder="Filter by name…" class="list-filter" />
  </div>
  <figure><table id="works-table"><thead><tr>
    <th class="sortable" data-sort="name">Name<span class="sort-ind"></span></th>
    <th class="sortable" data-sort="version">Version<span class="sort-ind"></span></th>
    <th>Steps</th><th>Description</th><th></th>
  </tr></thead><tbody></tbody></table></figure>
  <div id="works-pager"></div>`
}

function tasksSection(allTasks) {
  if (!allTasks.length) return '<p class="empty-state">No Tasks in this namespace.</p>'
  return `<div class="row-between" style="margin-bottom:0.75rem">
    <h3 style="margin:0">Tasks <span class="muted" id="tasks-count" style="font-size:0.85rem"></span></h3>
    <input type="search" id="tasks-filter" placeholder="Filter by name…" class="list-filter" />
  </div>
  <figure><table id="tasks-table"><thead><tr>
    <th class="sortable" data-sort="name">Name<span class="sort-ind"></span></th>
    <th class="sortable" data-sort="version">Version<span class="sort-ind"></span></th>
    <th>Executor</th><th>Description</th><th>In</th><th>Out</th>
  </tr></thead><tbody></tbody></table></figure>
  <div id="tasks-pager"></div>`
}

function wireWorks(allWorks, nsEnc) {
  if (!allWorks.length) return
  const ws = { q: '', sortKey: 'name', sortDir: 'asc', page: 1 }
  const SORT = { name: w => w.name, version: w => w.version }
  const draw = () => {
    const table = document.getElementById('works-table'); if (!table) return
    const filtered = applyFilter(allWorks, ws.q, w => w.name, w => w.version)
    const sorted   = applySort(filtered, SORT[ws.sortKey], ws.sortDir)
    const { slice, page, pages, total } = paginate(sorted, ws.page); ws.page = page
    const countEl = document.getElementById('works-count')
    if (countEl) countEl.textContent = total < allWorks.length ? `(${total} of ${allWorks.length})` : `(${total})`
    table.querySelector('tbody').innerHTML = slice.map(w => {
      const sc = w.spec?.steps?.length ?? 0; const key = esc(`${w.name}@${w.version}`)
      return `<tr><td><button class="plain-link work-detail-btn" data-work-key="${key}"><strong>${esc(w.name)}</strong></button></td>
        <td><span class="tag">${esc(w.version)}</span></td><td>${sc} step${sc!==1?'s':''}</td>
        <td class="muted">${esc(w.metadata?.description||'—')}</td>
        <td><a href="#/namespaces/${nsEnc}/workflows" role="button" class="outline btn-xs"
          data-work-name="${esc(w.name)}" data-work-version="${esc(w.version)}">▶ Run</a></td></tr>`
    }).join('')
    updateSortInds(table, ws)
    const pagerEl = document.getElementById('works-pager')
    if (pagerEl) { pagerEl.innerHTML = pagerHTML(page, pages, total); pagerEl.querySelectorAll('[data-page]').forEach(b => b.addEventListener('click', () => { ws.page = +b.dataset.page; draw() })) }
    table.querySelectorAll('.work-detail-btn').forEach(b => b.addEventListener('click', () => { const w = worksCache[b.dataset.workKey]; if (w) showWorkDetail(w) }))
    table.querySelectorAll('[data-work-name]').forEach(b => b.addEventListener('click', e => {
      e.preventDefault(); window.location.hash = `#/namespaces/${nsEnc}/workflows?new=${encodeURIComponent(b.dataset.workName)}&ver=${encodeURIComponent(b.dataset.workVersion)}`
    }))
  }
  document.getElementById('works-filter').addEventListener('input', e => { ws.q = e.target.value; ws.page = 1; draw() })
  document.getElementById('works-table').querySelectorAll('th[data-sort]').forEach(th => th.addEventListener('click', () => {
    const k = th.dataset.sort
    if (ws.sortKey === k) ws.sortDir = ws.sortDir === 'asc' ? 'desc' : 'asc'; else { ws.sortKey = k; ws.sortDir = 'asc' }
    ws.page = 1; draw()
  }))
  draw()
}

function wireTasks(allTasks) {
  if (!allTasks.length) return
  const ts = { q: '', sortKey: 'name', sortDir: 'asc', page: 1 }
  const SORT = { name: t => t.name, version: t => t.version }
  const tasksCache = Object.fromEntries(allTasks.map(t => [`${t.name}@${t.version}`, t]))
  const draw = () => {
    const table = document.getElementById('tasks-table'); if (!table) return
    const filtered = applyFilter(allTasks, ts.q, t => t.name, t => t.version)
    const sorted   = applySort(filtered, SORT[ts.sortKey], ts.sortDir)
    const { slice, page, pages, total } = paginate(sorted, ts.page); ts.page = page
    const countEl = document.getElementById('tasks-count')
    if (countEl) countEl.textContent = total < allTasks.length ? `(${total} of ${allTasks.length})` : `(${total})`
    table.querySelector('tbody').innerHTML = slice.map(t => {
      const key = esc(`${t.name}@${t.version}`)
      return `<tr style="cursor:pointer" data-task-key="${key}">
        <td><strong>${esc(t.name)}</strong></td><td><span class="tag">${esc(t.version)}</span></td>
        <td><code class="muted">${esc(t.spec?.execution?.kind??'—')}</code></td>
        <td class="muted">${esc(t.metadata?.description||'—')}</td>
        <td>${t.spec?.inputs?.length??0}</td><td>${t.spec?.outputs?.length??0}</td></tr>`
    }).join('')
    updateSortInds(table, ts)
    const pagerEl = document.getElementById('tasks-pager')
    if (pagerEl) { pagerEl.innerHTML = pagerHTML(page, pages, total); pagerEl.querySelectorAll('[data-page]').forEach(b => b.addEventListener('click', () => { ts.page = +b.dataset.page; draw() })) }
    table.querySelectorAll('tr[data-task-key]').forEach(row => row.addEventListener('click', () => { const t = tasksCache[row.dataset.taskKey]; if (t) showTaskDetail(t) }))
  }
  document.getElementById('tasks-filter').addEventListener('input', e => { ts.q = e.target.value; ts.page = 1; draw() })
  document.getElementById('tasks-table').querySelectorAll('th[data-sort]').forEach(th => th.addEventListener('click', () => {
    const k = th.dataset.sort
    if (ts.sortKey === k) ts.sortDir = ts.sortDir === 'asc' ? 'desc' : 'asc'; else { ts.sortKey = k; ts.sortDir = 'asc' }
    ts.page = 1; draw()
  }))
  draw()
}

function updateSortInds(table, state) {
  table.querySelectorAll('th[data-sort]').forEach(th => {
    const k = th.dataset.sort
    th.querySelector('.sort-ind').textContent = k === state.sortKey ? (state.sortDir === 'asc' ? ' ▲' : ' ▼') : ' ⇅'
    th.classList.toggle('sort-active', k === state.sortKey)
  })
}
