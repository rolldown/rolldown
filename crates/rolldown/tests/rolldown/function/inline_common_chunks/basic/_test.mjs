import assert from 'node:assert';

globalThis.events = [];
globalThis.markers = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepStrictEqual(globalThis.events, [
  'S body',
  'A 0 this:undefined 1',
  'B 1 this:undefined 2',
]);
assert.strictEqual(globalThis.markers.length, 2);
assert.strictEqual(
  globalThis.markers[0],
  globalThis.markers[1],
  'both entries see one shared instance',
);
