import assert from 'node:assert/strict';

globalThis.mergeGroupLog = [];
const main = await import('./dist/main.js');

assert.deepEqual(globalThis.mergeGroupLog, ['a-start', 'b', 'a-end', 'main']);
await main.loadA();
await main.loadB();
assert.deepEqual(globalThis.mergeGroupLog, ['a-start', 'b', 'a-end', 'main', 'route-a', 'route-b']);
