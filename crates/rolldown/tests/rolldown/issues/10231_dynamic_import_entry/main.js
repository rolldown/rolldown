import assert from 'node:assert';

export const ready = import('./lazy.js').then((m) => {
  assert.strictEqual(m.default, 42);
  assert.deepStrictEqual(Object.keys(m), ['default']);
});
