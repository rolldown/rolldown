import assert from 'node:assert/strict';

globalThis.configureCount = 0;
const { value } = await import('./dist/main.js');

assert.equal(value, 'used-value');
assert.equal(globalThis.configureCount, 1);
