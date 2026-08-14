import assert from 'node:assert/strict';

const load = async () => {
  import('./imp1').then((m) => {
    assert.strictEqual(m.imp1, 1);
  });
  import('./imp2').then((m) => {
    assert.strictEqual(m.imp2, 2);
  });
  import('./imp3').then((m) => {
    assert.deepEqual(
      // Workaround: When m is imported from a facade chunk, it has a null prototype
      Object.setPrototypeOf(m, null),
      Object.defineProperty(
        {
          __proto__: null,
          imp3: 3,
          imp33: 33,
        },
        Symbol.toStringTag,
        { value: 'Module' },
      ),
    );
  });
};

load();
