import assert from 'node:assert/strict';

globalThis.existingCycleLog = [];
const main = await import('./dist/main.js');

assert.deepEqual(globalThis.existingCycleLog, ['setup', 'lib', 'main']);
await main.load();
assert.deepEqual(globalThis.existingCycleLog, ['setup', 'lib', 'main']);
