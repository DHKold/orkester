// ── Modal dialog ───────────────────────────────────────────────────────────
//
// Uses the native <dialog> element defined in index.html.

const modal  = () => document.getElementById('modal')
const title  = () => document.getElementById('modal-title')
const body   = () => document.getElementById('modal-body')
const closeBtn = () => document.getElementById('modal-close-btn')

/**
 * Open the modal with the given title and HTML body content.
 * The caller should attach form/event listeners after calling this.
 */
export function openModal(titleText, html) {
  title().textContent = titleText
  body().innerHTML    = html
  closeBtn().onclick  = closeModal
  modal().onclick     = (e) => { if (e.target === modal()) closeModal() }
  modal().showModal()
}

export function closeModal() {
  modal().close()
}

/** Replace just the modal body (e.g. swap loading state for content). */
export function setModalBody(html) {
  body().innerHTML = html
}

/** Show a loading spinner inside the modal body. */
export function setModalLoading() {
  setModalBody('<p class="loading-state" aria-busy="true">Loading…</p>')
}
