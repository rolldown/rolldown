// nested-dynamic-import: outer.js is itself a lazy chunk and runs
// `import('./inner.js')` — a lazy import inside a lazy chunk. Regression for
// `hmr_ast_finalizer.rs::try_rewrite_dynamic_import`: without the fix,
// `inner.foo` was undefined. Only shows on the first click of a fresh page.
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
