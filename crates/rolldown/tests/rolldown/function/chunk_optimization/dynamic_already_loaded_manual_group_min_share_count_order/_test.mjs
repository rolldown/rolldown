import assert from 'node:assert/strict';

globalThis.minShareCountOrderLog = [];
const main = await import('./dist/main.js');

assert.deepEqual(globalThis.minShareCountOrderLog, ['setup', 'lib', 'main']);
assert.equal((await main.load()).v, 'lib');
