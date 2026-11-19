import assert from 'node:assert/strict';

globalThis.skip = true;
const { default: value } = await import('./dist/middle.js');
assert.equal(value, 0);
