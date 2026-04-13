const container = () => document.getElementById('toast-container')

function show(msg, type, duration = 4000) {
  const el = document.createElement('div')
  el.className = `toast toast--${type}`
  el.textContent = msg
  container().appendChild(el)
  setTimeout(() => el.remove(), duration)
}

export const toast        = (msg) => show(msg, 'info')
export const toastSuccess = (msg) => show(msg, 'success')
export const toastError   = (msg) => show(msg, 'error', 6000)
