import assert from 'node:assert/strict';

globalThis.events = [];
const { result } = await import('./dist/main.js');
assert.deepEqual(globalThis.events, ['eager', 'barrel', 'main', 'target']);
assert.deepEqual(result, [2, 1]);
