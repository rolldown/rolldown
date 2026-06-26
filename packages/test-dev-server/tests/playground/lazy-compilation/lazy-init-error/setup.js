// lazy-init-error: the lazily imported module throws during init. The error must
// surface at the consumer's `await import(...)` — not as an unhandled promise
// rejection from the proxy's eagerly-created `lazyExports` promise.
const log = (msg) => {
  document.getElementById('lazy-init-error-log').textContent += msg + '\n';
};

// Record any unhandled rejection so the spec can assert there were none.
window.addEventListener('unhandledrejection', (event) => {
  const reason = event.reason;
  const message = reason && reason.message ? reason.message : String(reason);
  document.getElementById('lazy-init-error-unhandled').textContent += message + '\n';
});

document.getElementById('lazy-init-error-btn').addEventListener('click', async () => {
  log('--- loading lazy-init-error (throws during init) ---');
  try {
    await import('./lazy-init-error.js');
    log('loaded');
  } catch (e) {
    log(`caught: ${e.message}`);
  }
  document.getElementById('lazy-init-error-status').textContent = 'done';
});
