const log = (msg) => {
  document.getElementById('log').textContent += msg + '\n';
};

log('app loaded.');

document.getElementById('btn').addEventListener('click', async () => {
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

  document.getElementById('status').textContent = 'done';
});
