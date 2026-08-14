import assert from 'node:assert';

export const foo = 'foo';

// this generation's callback fires when the FIRST edit applies, with the new exports
import.meta.hot.accept((mod) => {
  assert.strictEqual(mod.foo, 'foo2');
});
