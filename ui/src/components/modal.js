const modal  = () => document.getElementById('modal')
const title  = () => document.getElementById('modal-title')
const body   = () => document.getElementById('modal-body')
const close  = () => document.getElementById('modal-close-btn')

/**
 * Open the modal with given title and HTML content.
 * The caller is responsible for attaching form event listeners after calling this.
 */
export function openModal(titleText, html) {
  title().textContent = titleText
  body().innerHTML = html
  close().onclick = closeModal
  // Close on backdrop click
  modal().onclick = (e) => { if (e.target === modal()) closeModal() }
  modal().showModal()
}

export function closeModal() {
  modal().close()
}

/** Replace just the modal body content (e.g. to show loading state). */
export function setModalBody(html) {
  body().innerHTML = html
}
