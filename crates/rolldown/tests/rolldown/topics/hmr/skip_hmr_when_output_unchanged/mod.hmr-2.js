import assert from 'node:assert';

export const value = 'world';

import.meta.hot.accept((mod) => {
  assert.strictEqual(mod.value, 'world');
});
