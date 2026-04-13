// ── Log viewer ─────────────────────────────────────────────────────────────
//
// Renders a tabbed stdout / stderr viewer, fetching content from artifact URIs.
//
// Usage:
//   document.getElementById('logs-wrap').innerHTML = renderLogViewer('task-logs', logsRef)
//   await bindLogViewer('task-logs', logsRef)
//   // logsRef: { stdout?: string, stderr?: string } — URI strings

import { fetchArtifact } from '../api.js'
import { esc }           from '../utils.js'

/**
 * Return the HTML shell for a log viewer.
 * bindLogViewer() must be called after this is in the DOM.
 *
 * @param {string} id      - Base ID for the viewer elements
 * @param {Object} logsRef - { stdout?, stderr? } artifact URIs
 */
export function renderLogViewer(id, logsRef = {}) {
  const hasSub = !!logsRef.stdout
  const hasErr = !!logsRef.stderr
  const tabs   = hasSub || hasErr

  return `
    <div class="log-wrapper" id="${id}">
      ${tabs ? `
        <div class="log-tabs">
          ${hasSub ? `<button class="log-tab-btn active" data-log-tab="${id}-stdout">stdout</button>` : ''}
          ${hasErr ? `<button class="log-tab-btn${!hasSub ? ' active' : ''}" data-log-tab="${id}-stderr">stderr</button>` : ''}
        </div>` : ''}
      <pre class="log-viewer" id="${id}-stdout">Loading…</pre>
      ${hasErr ? `<pre class="log-viewer" id="${id}-stderr" style="display:none">Loading…</pre>` : ''}
    </div>`
}

/**
 * Fetch log content and wire tab switching.
 * Safe to call multiple times (re-fetches on each call).
 *
 * @param {string} id
 * @param {Object} logsRef - { stdout?, stderr? }
 */
export async function bindLogViewer(id, logsRef = {}) {
  const stdoutEl = document.getElementById(`${id}-stdout`)
  const stderrEl = document.getElementById(`${id}-stderr`)
  const wrapper  = document.getElementById(id)
  if (!wrapper) return

  // Tab switching
  wrapper.querySelectorAll('.log-tab-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      wrapper.querySelectorAll('.log-tab-btn').forEach(b => b.classList.remove('active'))
      btn.classList.add('active')
      const targetId = btn.dataset.logTab
      wrapper.querySelectorAll('.log-viewer').forEach(pre => {
        pre.style.display = pre.id === targetId ? '' : 'none'
      })
    })
  })

  // Fetch stdout
  if (stdoutEl && logsRef.stdout) {
    try {
      const text = await fetchArtifact(logsRef.stdout)
      stdoutEl.textContent = text || '(empty)'
      stdoutEl.scrollTop   = stdoutEl.scrollHeight
    } catch (e) {
      stdoutEl.textContent = `Error fetching logs: ${e.message}`
    }
  } else if (stdoutEl) {
    stdoutEl.textContent = '(no stdout)'
  }

  // Fetch stderr
  if (stderrEl && logsRef.stderr) {
    try {
      const text = await fetchArtifact(logsRef.stderr)
      stderrEl.textContent = text || '(empty)'
    } catch (e) {
      stderrEl.textContent = `Error fetching logs: ${e.message}`
    }
  } else if (stderrEl) {
    stderrEl.textContent = '(no stderr)'
  }
}

/**
 * Inline log viewer for structured log entries (WorkRun.status.logs[]).
 * @param {Array} logs - [{ timestamp, level, message }]
 * @returns {string} HTML
 */
export function renderStructuredLogs(logs = []) {
  if (!logs.length) return '<pre class="log-viewer">(no logs)</pre>'
  const COLORS = { error: '#f87171', warn: '#fbbf24', info: '#94a3b8', debug: '#64748b' }
  const lines = logs.map(entry => {
    const ts    = entry.timestamp ? new Date(entry.timestamp).toISOString().slice(11, 23) : ''
    const level = (entry.level ?? 'info').toLowerCase()
    const color = COLORS[level] ?? COLORS.info
    const msg   = esc(entry.message ?? JSON.stringify(entry))
    const tsHtml  = ts    ? `<span style="color:#475569">${esc(ts)}</span> ` : ''
    const lvlHtml = level ? `<span style="color:${color}">[${esc(level.toUpperCase())}]</span> ` : ''
    return `${tsHtml}${lvlHtml}${msg}`
  }).join('\n')
  return `<pre class="log-viewer">${lines}</pre>`
}
