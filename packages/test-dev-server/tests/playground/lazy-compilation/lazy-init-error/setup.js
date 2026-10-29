// lazy-init-error: the lazily imported `./lazy-init-error.js` throws while
// initializing, so rolldown's lazy proxy must reproduce the browser's native
// behavior for a dynamic import of a throwing module:
//
//   - WITH try/catch:   the error is caught at `await import(...)`.
//   - WITHOUT a handler: it surfaces as a single `unhandledrejection`.
//
// Before the fix, the init error escaped the proxy's eagerly-created export
// promise as an unhandled rejection and the consumer's `await import(...)`
// resolved as if nothing went wrong (vitejs/vite#21626).
const log = (msg) => {
  document.getElementById('lazy-init-error-log').textContent += msg + '\n';
};

// Record unhandled rejections so the spec can assert whether one escaped.
window.addEventListener('unhandledrejection', (event) => {
  const reason = event.reason;
  const message = reason && reason.message ? reason.message : String(reason);
  document.getElementById('lazy-init-error-unhandled').textContent += message + '\n';
});

document.getElementById('lazy-init-error-catch-btn').addEventListener('click', async () => {
  log('--- lazy import WITH try/catch ---');
  try {
    await import('./lazy-init-error.js');
    log('resolved');
  } catch (e) {
    log(`caught: ${e.message}`);
  }
  document.getElementById('lazy-init-error-status').textContent = 'catch-done';
});

document.getElementById('lazy-init-error-nocatch-btn').addEventListener('click', () => {
  log('--- lazy import WITHOUT a handler ---');
  // Fire and forget: no await, no `.catch`. The rejection must go unhandled.
  import('./lazy-init-error.js');
  document.getElementById('lazy-init-error-status').textContent = 'nocatch-done';
});
