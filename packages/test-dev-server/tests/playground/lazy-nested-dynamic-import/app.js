const log = (msg) => {
  document.getElementById('log').textContent += msg + '\n';
};

log('app loaded.');

document.getElementById('btn').addEventListener('click', async () => {
  log('--- loading outer (lazy chunk) ---');
  const outer = await import('./outer.js');
  log(`outer.outerName = ${outer.outerName}`);

  log('--- triggering nested dynamic import (lazy -> lazy) ---');
  const inner = await outer.loadInner();
  log(`inner.foo = ${inner.foo === undefined ? 'UNDEFINED' : inner.foo}`);
  log(`inner.bar = ${inner.bar === undefined ? 'UNDEFINED' : inner.bar}`);

  document.getElementById('status').textContent = 'done';
});
