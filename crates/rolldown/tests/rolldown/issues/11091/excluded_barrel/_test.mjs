import assert from 'node:assert/strict';

const { value } = await import('./dist/bundle.mjs');

assert.deepEqual(value, { answer: 42 });
