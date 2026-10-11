import assert from 'node:assert/strict';

globalThis.marker = 'before';
const { result, read } = await import('./dist/main.js');
assert.equal(result, 'local');
assert.equal(await read(), 'before');
