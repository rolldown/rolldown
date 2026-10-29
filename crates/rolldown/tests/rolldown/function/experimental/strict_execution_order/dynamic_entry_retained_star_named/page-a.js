import assert from 'node:assert';

export * from './bridge.js';

assert.strictEqual(
  globalThis.value,
  0,
  'named retained-star re-export must initialize common before page-a',
);
