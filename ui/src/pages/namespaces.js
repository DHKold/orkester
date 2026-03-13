import { listNamespaces } from '../api.js'
import { esc, setApp } from '../utils.js'
import { toastError } from '../components/toast.js'

export async function renderNamespaces() {
  setApp('<p aria-busy="true">Loading namespaces…</p>')

  try {
    const data = await listNamespaces()
    const namespaces = data.namespaces ?? []

    if (namespaces.length === 0) {
      setApp(`
        <hgroup><h2>Namespaces</h2></hgroup>
        <div class="empty-state">
          <p>No namespaces loaded. Add YAML definitions to your workspace loader directory.</p>
        </div>
      `)
      return
    }

    const cards = namespaces.map(ns => {
      const name = esc(ns.name)
      const desc = esc(ns.metadata?.description || '')
      const nsEnc = encodeURIComponent(ns.name)
      return `
        <div>
          <article>
            <header>
              <strong>${name}</strong>
              <span class="muted" style="float:right;font-size:0.8rem">${esc(ns.version ?? '')}</span>
            </header>
            ${desc ? `<p class="muted">${desc}</p>` : '<p class="muted">No description</p>'}
            <footer style="display:flex;gap:0.5rem;flex-wrap:wrap">
              <a href="#/namespaces/${nsEnc}" role="button" class="secondary outline btn-xs">Catalog</a>
              <a href="#/namespaces/${nsEnc}/workflows" role="button" class="outline btn-xs">Workflows</a>
              <a href="#/namespaces/${nsEnc}/crons" role="button" class="outline btn-xs">Crons</a>
            </footer>
          </article>
        </div>
      `
    }).join('')

    setApp(`
      <hgroup>
        <h2>Namespaces</h2>
        <p>${namespaces.length} namespace${namespaces.length !== 1 ? 's' : ''} available</p>
      </hgroup>
      <div class="namespace-grid">${cards}</div>
    `)
  } catch (e) {
    toastError(`Failed to load namespaces: ${e.message}`)
    setApp('<div class="empty-state"><p>Failed to load namespaces.</p></div>')
  }
}
