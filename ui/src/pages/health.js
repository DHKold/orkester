import { getHealth, getServers, getPlugins } from '../api.js'
import { esc, fmtUptime, setApp } from '../utils.js'
import { toastError } from '../components/toast.js'

export async function renderHealth() {
  setApp('<p aria-busy="true">Loading…</p>')

  try {
    const [health, servers, plugins] = await Promise.all([
      getHealth(), getServers(), getPlugins()
    ])

    const statusClass = health.status === 'ok' ? 'succeeded' : 'failed'

    const serverRows = (servers.servers ?? []).map(s => `
      <tr>
        <td>${esc(s.instance_name)}</td>
        <td><code>${esc(s.component)}</code></td>
      </tr>
    `).join('')

    const pluginRows = (plugins.plugins ?? []).map(p => `
      <tr>
        <td><strong>${esc(p.id)}</strong></td>
        <td>${esc(p.version)}</td>
        <td>${esc(p.description)}</td>
        <td class="muted">${(p.authors ?? []).map(esc).join(', ')}</td>
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
          <div class="metric-value">${esc(fmtUptime(health.uptime_seconds))}</div>
          <div class="metric-label">Uptime</div>
        </div>
      </div>

      <article>
        <header><strong>Servers</strong></header>
        <figure>
          <table>
            <thead><tr><th>Name</th><th>Component</th></tr></thead>
            <tbody>${serverRows || '<tr><td colspan="2" class="muted">No servers</td></tr>'}</tbody>
          </table>
        </figure>
      </article>

      <article>
        <header><strong>Plugins</strong></header>
        <figure>
          <table>
            <thead><tr><th>ID</th><th>Version</th><th>Description</th><th>Authors</th></tr></thead>
            <tbody>${pluginRows || '<tr><td colspan="4" class="muted">No plugins</td></tr>'}</tbody>
          </table>
        </figure>
      </article>
    `)
  } catch (e) {
    toastError(`Failed to load health: ${e.message}`)
    setApp('<div class="empty-state"><p>Could not reach Orkester. Check the API base URL.</p></div>')
  }
}
