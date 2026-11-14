import assert from 'node:assert';

export const foo = 'foo3';

// the last generation: no further edit ever applies, so this must never fire
import.meta.hot.accept(() => {
  assert.fail('the last generation has no further edit to accept');
});
