import assert from 'node:assert';
export const value = 'hello';
import.meta.hot.accept((mod) => {
  assert.strictEqual(mod.value, 'hello');
});
