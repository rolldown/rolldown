// new-url: a lazily-imported module that references an asset via
// `new URL('./img', import.meta.url)` instead of a static `import`. This goes
// through a DIFFERENT resolution path than `emitted-asset` — the core scanner's
// `ImportKind::NewUrl` record + the module finalizer's rewrite, not the
// `__ROLLDOWN_ASSET__` placeholder. The HMR/lazy codegen uses its own finalizer
// that performs neither, so this exercises a separate facet of the same gap
// (rolldown#9812).
const log = (msg) => {
  document.getElementById('new-url-log').textContent += msg + '\n';
};

document.getElementById('new-url-status').textContent = 'ready';

document.getElementById('new-url-btn').addEventListener('click', async () => {
  log('--- loading new URL module (lazy compiled) ---');
  await import('./lazy.js');
  document.getElementById('new-url-status').textContent = 'loaded';
});
