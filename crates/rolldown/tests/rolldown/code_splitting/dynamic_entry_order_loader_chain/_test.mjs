import assert from 'node:assert/strict';
const { value } = await import('./dist/main.js');
assert.equal(value, 'ok');
