import assert from 'node:assert';

export const value = 'world';

import.meta.hot.accept((mod) => {
  // Never fires: there is no further update after step 2.
  assert.strictEqual(mod.value, 'world');
});
