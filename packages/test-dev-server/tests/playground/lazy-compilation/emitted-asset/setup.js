// emitted-asset: a dynamic import whose module pulls in an image that exists in
// no other module, so the asset is emitted only when this lazy chunk is
// compiled on the first click. Regression for vitejs/vite#22596 — the lazy
// patch must reference the asset's resolved URL (not a raw `__ROLLDOWN_ASSET__`
// placeholder) and that URL must be served on the first request, not only after
// a refresh.
const log = (msg) => {
  document.getElementById('emitted-asset-log').textContent += msg + '\n';
};

document.getElementById('emitted-asset-status').textContent = 'ready';

document.getElementById('emitted-asset-btn').addEventListener('click', async () => {
  log('--- loading asset module (lazy compiled) ---');
  await import('./lazy.js');
  document.getElementById('emitted-asset-status').textContent = 'loaded';
});
