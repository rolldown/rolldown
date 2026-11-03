import assert from 'node:assert/strict';

globalThis.effects = [];
await import('./dist/instanceof.js');
assert.deepEqual(globalThis.effects, ['instanceof']);

await assert.rejects(import('./dist/typed-array.js'), RangeError);
await assert.rejects(import('./dist/array-buffer.js'), RangeError);
await assert.rejects(import('./dist/bigint-shift.js'), RangeError);
await assert.rejects(import('./dist/bigint-power.js'), RangeError);
