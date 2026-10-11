import assert from 'node:assert/strict';

globalThis.orderValue = 'before';
const { loadA, loadB } = await import('./dist/main.js');
assert.equal((await loadA()).AValue, 'before');
assert.equal((await loadB()).AValue, 'before');
