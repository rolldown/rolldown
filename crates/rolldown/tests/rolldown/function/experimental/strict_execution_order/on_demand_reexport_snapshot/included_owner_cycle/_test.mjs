import assert from 'node:assert/strict';

globalThis.events = [];
const { loadA, loadB } = await import('./dist/main.js');
const first = await loadB();
assert.equal(first.CValue, 1);
assert.equal(first.DValue, 2);
assert.equal(first.readD(), 2);
assert.deepEqual(globalThis.events, ['c', 'd']);
assert.equal((await loadA()).CValue, 1);
assert.deepEqual(globalThis.events, ['c', 'd']);
