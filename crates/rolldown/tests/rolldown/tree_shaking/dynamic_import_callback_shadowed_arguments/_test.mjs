import assert from 'node:assert/strict';
const { default: load } = await import('./dist/main.cjs');
assert.equal(await load(), 'expected');
