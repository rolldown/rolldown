// basic: a single dynamic import compiles into its own chunk(s) on first
// click. The spec counts the `lazy-module` requests to show the proxy chunk
// and the real chunk are fetched separately — eager bundling would not do that.
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
