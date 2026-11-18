import assert from 'node:assert/strict';

const { copy } = await import('./dist/bundle.mjs');

assert.deepEqual(copy, { answer: 42 });
