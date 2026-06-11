// aliased-import: `import('@lazy')` resolves through `viteAliasPlugin` to
// `lazy.js`. Regression for vitejs/vite#22454 — the aliased proxy used to carry
// `?rolldown-lazy=1` twice, so the real module never registered its named
// exports and `mod.foo`/`mod.bar` came back undefined. The bug only shows on the
// first click of a virgin server, so the spec uses `{ retry: 0 }`.
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
