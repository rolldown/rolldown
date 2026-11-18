import assert from 'node:assert/strict';
const m = await import('./dist/main.js');
assert.equal(m.value, 'Pq');
