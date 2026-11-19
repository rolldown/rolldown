import assert from 'node:assert/strict';

const { load } = await import('./dist/main.js');
assert.deepEqual(Object.keys(await load()), []);
