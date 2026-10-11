import assert from 'node:assert/strict';

globalThis.events = [];
const { loadA, loadB } = await import('./dist/main.js');
const first = await loadA();
assert.equal(first.CValue, 2);
assert.equal(first.DValue, 1);
assert.equal(first.readD(), 1);
assert.deepEqual(globalThis.events, ['d', 'c']);
assert.equal((await loadB()).DValue, 1);
assert.deepEqual(globalThis.events, ['d', 'c']);
