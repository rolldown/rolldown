import assert from 'node:assert/strict';

const main = await import('./dist/main.js');

assert.deepEqual(Object.keys(main), ['default']);
assert.equal(await main.default(), 'liblib');
