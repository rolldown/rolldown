import assert from 'node:assert';
export const value = 'hello';
import.meta.hot.accept((mod) => {
  // The callback registered by one run fires at the next shipped update, with
  // that update's exports. Steps 0 and 1 are suppressed as no-ops, so this
  // fires exactly once: at step 2, whose value is 'world'. If a no-op step
  // regressed into shipping a patch, this would fire early with 'hello'.
  assert.strictEqual(mod.value, 'world');
});
