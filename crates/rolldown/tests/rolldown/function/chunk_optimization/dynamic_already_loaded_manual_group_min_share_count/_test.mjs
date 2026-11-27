import assert from 'node:assert/strict';

globalThis.minShareCountLog = [];
const a = await import('./dist/a.js');
await import('./dist/b.js');

assert.deepEqual(globalThis.minShareCountLog, ['peer:ok', 'a', 'b']);
const [lazy, other] = await a.load();
assert.equal(lazy.value, 'ok');
assert.equal(other.other, 'ok');
