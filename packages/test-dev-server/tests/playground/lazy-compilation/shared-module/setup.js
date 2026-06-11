// shared-module: page-a and page-b each statically import selectors, so
// selectors lands in a `ChunkKind::Common` chunk where export keys get aliased.
// Regression for PR #9132 — the fetched proxy must read exports from the
// runtime registry (`loadExports`) instead of the raw chunk namespace, otherwise
// `sel.foo` is undefined. Only manifests on the first click of a virgin server.
const log = (msg) => {
  document.getElementById('shared-module-log').textContent += msg + '\n';
};

document.getElementById('shared-module-btn').addEventListener('click', async () => {
  log('--- loading page-a ---');
  const a = await import('./page-a.js');
  log(`page-a.a = ${a.a}`);

  log('--- loading page-b ---');
  const b = await import('./page-b.js');
  log(`page-b.b = ${b.b}`);

  log('--- loading selectors directly ---');
  const sel = await import('./selectors.js');
  log(`sel.foo = ${sel.foo === undefined ? 'UNDEFINED' : sel.foo}`);
  log(`sel.bar = ${sel.bar === undefined ? 'UNDEFINED' : sel.bar}`);

  document.getElementById('shared-module-status').textContent = 'done';
});
