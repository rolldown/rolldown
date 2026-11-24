import assert from 'node:assert/strict';

globalThis.orderStraddleLog = [];
const main = await import('./dist/main.js');

assert.deepEqual(globalThis.orderStraddleLog, ['a', 'x', 't']);
await main.loadD();
assert.deepEqual(globalThis.orderStraddleLog, ['a', 'x', 't']);
