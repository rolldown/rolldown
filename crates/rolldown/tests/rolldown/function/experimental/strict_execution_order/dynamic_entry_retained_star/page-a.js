import assert from 'node:assert';

export * from './bridge.js';

assert.strictEqual(globalThis.value, 0);
