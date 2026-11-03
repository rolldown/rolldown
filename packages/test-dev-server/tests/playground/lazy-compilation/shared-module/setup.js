// shared-module: page-a and page-b both import selectors, so selectors lands
// in a shared chunk where export names get minified. Regression for PR #9132:
// a lazy import must resolve to the exports read by stable id, otherwise
// `sel.foo` is undefined. Only shows on the first click of a fresh server.
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

// route-a and route-b both import store. The spec visits A, reloads, then visits B before
// A, so A is the route that was compiled before. store must still run once per page load.
const loaded = {};
const showStores = () => {
  document.getElementById('shared-module-stores').textContent =
    `a:${loaded.a?.run ?? '-'} b:${loaded.b?.run ?? '-'} same:${loaded.a === loaded.b}`;
};
document.getElementById('shared-module-a-btn').addEventListener('click', async () => {
  loaded.a = (await import('./route-a.js')).storeA;
  showStores();
});
document.getElementById('shared-module-b-btn').addEventListener('click', async () => {
  loaded.b = (await import('./route-b.js')).storeB;
  showStores();
});
