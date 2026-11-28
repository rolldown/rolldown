import assert from 'node:assert/strict';

globalThis.wrappedCjsLog = [];
const e0 = await import('./dist/e0.js');

assert.deepEqual(globalThis.wrappedCjsLog, ['t', 'b2', 'x', 'e0']);
assert.equal(await e0.loadB0(), 'b0');
