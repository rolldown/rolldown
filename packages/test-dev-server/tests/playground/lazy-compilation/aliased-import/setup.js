// aliased-import: `import('@lazy')` resolves through `viteAliasPlugin` to
// `lazy.js`. Regression for vitejs/vite#22454 — the proxy id used to get the
// `?rolldown-lazy=1` suffix twice, so `mod.foo`/`mod.bar` were undefined.
// Only shows on the first click of a fresh server, hence `{ retry: 0 }`.
const log = (msg) => {
  document.getElementById('aliased-import-log').textContent += msg + '\n';
};

document.getElementById('aliased-import-btn').addEventListener('click', async () => {
  log('--- loading @lazy (lazy chunk via alias) ---');
  const mod = await import('@lazy');
  log(`mod.foo = ${mod.foo === undefined ? 'UNDEFINED' : mod.foo}`);
  log(`mod.bar = ${mod.bar === undefined ? 'UNDEFINED' : mod.bar}`);
  document.getElementById('aliased-import-status').textContent = 'done';
});
