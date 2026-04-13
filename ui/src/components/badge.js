// ── Status badge helper ────────────────────────────────────────────────────
import { esc } from '../utils.js'

/**
 * Return an HTML string for a status badge.
 * The CSS class `badge--{status}` drives the colour (see components.css).
 */
export function badge(status) {
  if (!status) return '—'
  return `<span class="badge badge--${esc(status)}">${esc(status)}</span>`
}

/** True if the run state is terminal (no further polling needed). */
export function isTerminalState(state) {
  return ['succeeded', 'failed', 'cancelled', 'dropped'].includes(state)
}
