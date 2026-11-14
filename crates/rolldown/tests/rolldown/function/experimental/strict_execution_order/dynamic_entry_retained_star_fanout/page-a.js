import assert from 'node:assert';

export * from './bridge.js';

assert.strictEqual(globalThis.valueA, 0);
assert.strictEqual(globalThis.valueB, 0);
