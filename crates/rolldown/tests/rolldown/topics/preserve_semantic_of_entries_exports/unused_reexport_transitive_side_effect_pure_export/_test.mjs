import assert from 'node:assert/strict';

globalThis.entryLog = [];
globalThis.initialized = undefined;
await import('./dist/main.js');

assert.equal(globalThis.startupValue, 1);
assert.deepEqual(globalThis.entryLog, ['effect', 'entry']);
assert.equal(await globalThis.loadLazy(), 1);
assert.deepEqual(globalThis.entryLog, ['effect', 'entry']);
