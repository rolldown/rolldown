import assert from 'node:assert';

export const ready = Promise.all([
  import('./lazy.js').then((m) => {
    assert.strictEqual(m.default, 42);
    assert.deepStrictEqual(Object.keys(m), ['default']);
  }),
  import('./lazy-cjs.cjs').then((m) => {
    assert.deepStrictEqual(m.default, { a: 1 });
    assert.strictEqual(m.a, 1);
  }),
]);
