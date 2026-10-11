import assert from 'node:assert/strict';

globalThis.orderValue = 'before';
const { loadA, loadB } = await import('./dist/main.js');
assert.equal((await loadB()).AValue, 'after');
assert.equal((await loadA()).AValue, 'after');
