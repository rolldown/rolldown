// basic: a single dynamic import is lazy-compiled into its own chunk(s) on first
// click. The spec counts the `lazy-module` requests to prove the proxy chunk +
// real chunk are fetched separately (which eager bundling would not produce).
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
