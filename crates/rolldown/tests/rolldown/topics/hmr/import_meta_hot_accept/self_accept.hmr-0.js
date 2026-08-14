import assert from 'node:assert';

export const foo = 'foo2';

// fires when the second edit applies
import.meta.hot.accept((mod) => {
  assert.strictEqual(mod.foo, 'foo3');
});
