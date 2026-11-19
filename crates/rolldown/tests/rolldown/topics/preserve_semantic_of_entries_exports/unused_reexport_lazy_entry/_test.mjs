import assert from 'node:assert/strict';

globalThis.entryLog = [];
await import('./dist/main.js');
assert.deepEqual(globalThis.entryLog, ['entry runs']);

const namespace = await globalThis.loadD();
assert.deepEqual(globalThis.entryLog, ['entry runs', 'd runs q']);
assert.deepEqual(Object.keys(namespace), ['d']);
assert.equal(namespace.d, 1);
