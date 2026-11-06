import assert from 'node:assert/strict';

let initialized = 0;
globalThis.React = {
  memo(callback) {
    initialized++;
    return callback;
  },
};
try {
  await import('./dist/main.js');
  assert.equal(initialized, 1);
  await import('./dist/main.js');
  assert.equal(initialized, 1);
} finally {
  delete globalThis.React;
}
