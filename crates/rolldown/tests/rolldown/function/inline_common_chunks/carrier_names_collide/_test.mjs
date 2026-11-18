import assert from 'node:assert';

globalThis.events = [];
globalThis.markers = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepStrictEqual(globalThis.events, [
  'A vendor1 1 entry-helper entry-init entry-count$1 100',
  'B vendor2 2 entry-b-helper',
]);
assert.strictEqual(globalThis.markers[0], globalThis.markers[1]);
