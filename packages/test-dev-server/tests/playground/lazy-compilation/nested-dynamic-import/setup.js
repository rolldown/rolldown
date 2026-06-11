// nested-dynamic-import: outer.js is itself a lazy chunk, and its body runs
// `import('./inner.js')` — a lazy import inside a lazy chunk. Regression for the
// HMR AST finalizer fix in `hmr_ast_finalizer.rs::try_rewrite_dynamic_import`
// (the `?rolldown-lazy=1` branch): the inner import must resolve to the proxy
// module's registered exports (carrying `'rolldown:exports'`), otherwise
// `inner.foo` came back undefined. Only manifests on the first click of a fresh
// page.
const log = (msg) => {
  document.getElementById('nested-dynamic-import-log').textContent += msg + '\n';
};

document.getElementById('nested-dynamic-import-btn').addEventListener('click', async () => {
  log('--- loading outer (lazy chunk) ---');
  const outer = await import('./outer.js');
  log(`outer.outerName = ${outer.outerName}`);

  log('--- triggering nested dynamic import (lazy -> lazy) ---');
  const inner = await outer.loadInner();
  log(`inner.foo = ${inner.foo === undefined ? 'UNDEFINED' : inner.foo}`);
  log(`inner.bar = ${inner.bar === undefined ? 'UNDEFINED' : inner.bar}`);

  document.getElementById('nested-dynamic-import-status').textContent = 'done';
});
