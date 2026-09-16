// basic: one lazy import, compiled by the server on first click in one request.
const log = (msg) => {
  document.getElementById('basic-log').textContent += msg + '\n';
};

document.getElementById('basic-status').textContent = 'main loaded';

document.getElementById('basic-btn').addEventListener('click', async () => {
  log('--- loading lazy-module (lazy compiled) ---');
  const lazyModule = await import('./lazy-module.js');
  log(`value = ${lazyModule.value}`);
  document.getElementById('basic-status').textContent = lazyModule.value;
});
