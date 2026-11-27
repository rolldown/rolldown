import assert from 'node:assert/strict';

globalThis.manualGroupCycleLog = [];
const main = await import('./dist/main.js');

assert.deepEqual(globalThis.manualGroupCycleLog, ['a', 'a']);
assert.equal(await main.loadLazy(), 'a');
