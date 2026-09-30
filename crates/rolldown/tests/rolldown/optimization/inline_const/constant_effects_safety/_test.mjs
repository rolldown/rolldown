import assert from 'node:assert/strict';

const log = console.log;
const calls = [];
console.log = (value) => calls.push(value);
globalThis.mutable = true;
try {
  await import('./dist/main.js');
} finally {
  console.log = log;
  delete globalThis.mutable;
}
assert.deepEqual(calls, ['TEST_EFFECT', 'LIVE_ELSE', 'LIVE_TRUE', 'MUTABLE', 'COERCION']);
