const log = (msg) => {
  document.getElementById('log').textContent += msg + '\n';
};

log('app loaded.');

document.getElementById('btn').addEventListener('click', async () => {
  log('--- loading @lazy (lazy chunk via alias) ---');
  const mod = await import('@lazy');
  log(`mod.foo = ${mod.foo === undefined ? 'UNDEFINED' : mod.foo}`);
  log(`mod.bar = ${mod.bar === undefined ? 'UNDEFINED' : mod.bar}`);

  document.getElementById('status').textContent = 'done';
});
