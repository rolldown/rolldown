import assert from 'node:assert';

globalThis.__events = [];

const { loadPage } = await import('./dist/main.js');
await loadPage();

// The definer is reachable only through barrel-outer -> barrel-inner -> definer, and the reader
// that consumes it lives inside the dynamically imported chunk. The wrap-all cell exercises init
// forwarding through both barrel wrappers; on-demand may keep this particular graph eager. In both
// modes the definer must run before the reader reads `value`. A regression in wrapper forwarding
// left the inner barrel's `init_*` empty, so `value` read `undefined` (issue family #8777 / #8989).
assert.ok(globalThis.__events.includes('definer'), 'definer module must have been initialized');
assert.ok(
  globalThis.__events.includes('reader:5'),
  `reader must read the initialized definer value; got ${JSON.stringify(globalThis.__events)}`,
);
assert.ok(
  globalThis.__events.indexOf('definer') < globalThis.__events.indexOf('reader:5'),
  'definer must initialize before the reader reads it',
);
assert.deepStrictEqual(globalThis.__events, [
  'main',
  'leaf',
  'definer',
  'reader:5',
  'reader-host:105',
  'dynamic-page',
]);
