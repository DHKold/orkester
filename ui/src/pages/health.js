import { getHealth, getHostPlugins, getHostComponents, getHostRegistry } from '../api.js'
import { esc, setApp, fmtUptime } from '../utils.js'
import { toastError } from '../components/toast.js'

export async function renderHealth() {
  setApp('<p aria-busy="true">Loading…</p>')

  try {
    const [health, plugins, components, registry] = await Promise.all([
      getHealth(),
      getHostPlugins().catch(() => []),
      getHostComponents().catch(() => []),
      getHostRegistry().catch(() => []),
    ])

    const statusClass = health.status === 'ok' ? 'succeeded' : 'failed'

    const metaRows = (Array.isArray(plugins) ? plugins : []).map(p => `
      <tr>
        <td><code>${esc(p.kind)}</code></td>
        <td>${esc(p.name)}</td>
        <td class="muted">${esc(p.description ?? '')}</td>
      </tr>
    `).join('')

    const componentRows = (Array.isArray(components) ? components : []).map(c => `
      <tr>
        <td><code>${esc(c.kind)}</code></td>
        <td>${esc(c.name)}</td>
        <td class="muted">${esc(c.description ?? '')}</td>
      </tr>
    `).join('')

    const registryRows = (Array.isArray(registry) ? registry : []).map(r => `
      <tr>
        <td>${esc(r.name)}</td>
        <td><code>${esc(r.kind)}</code></td>
      </tr>
    `).join('')

    setApp(`
      <hgroup>
        <h2>System Health</h2>
        <p>Platform status and loaded components</p>
      </hgroup>

      <div class="metrics-grid" style="max-width:400px">
        <div class="metric-card">
          <div class="metric-value">
            <span class="badge badge--${statusClass}">${esc(health.status)}</span>
          </div>
          <div class="metric-label">Status</div>
        </div>
        <div class="metric-card">
          <div class="metric-value">${esc(fmtUptime(health.uptime_secs))}</div>
          <div class="metric-label">Uptime</div>
        </div>
        <div class="metric-card">
          <div class="metric-value">${esc(health.version)}</div>
          <div class="metric-label">Version</div>
        </div>
      </div>

      <article>
        <header><strong>Plugin Catalogue</strong> &mdash; component types available for instantiation</header>
        <figure>
          <table>
            <thead><tr><th>Kind</th><th>Name</th><th>Description</th></tr></thead>
            <tbody>${metaRows || '<tr><td colspan="3" class="muted">No plugins loaded</td></tr>'}</tbody>
          </table>
        </figure>
      </article>

      <article>
        <header><strong>Component Types</strong> &mdash; all registered component types</header>
        <figure>
          <table>
            <thead><tr><th>Kind</th><th>Name</th><th>Description</th></tr></thead>
            <tbody>${componentRows || '<tr><td colspan="3" class="muted">None</td></tr>'}</tbody>
          </table>
        </figure>
      </article>

      <article>
        <header><strong>Running Instances</strong> &mdash; live component registry</header>
        <figure>
          <table>
            <thead><tr><th>Name</th><th>Kind</th></tr></thead>
            <tbody>${registryRows || '<tr><td colspan="2" class="muted">No instances</td></tr>'}</tbody>
          </table>
        </figure>
      </article>
    `)
  } catch (e) {
    toastError(`Failed to load health: ${e.message}`)
    setApp('<div class="empty-state"><p>Could not reach Orkester. Check the API base URL.</p></div>')
  }
}
