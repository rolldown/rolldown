import assert from 'node:assert';

globalThis.events = [];
globalThis.markers = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepStrictEqual(globalThis.events, ['CJS body', 'A 1 true', 'B 2 3']);
assert.strictEqual(globalThis.markers[0], globalThis.markers[1], 'one module.exports object');
