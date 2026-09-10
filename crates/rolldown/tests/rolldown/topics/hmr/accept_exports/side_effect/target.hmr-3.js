import assert from 'node:assert';

export const a = 1;

import.meta.hot.acceptExports([], () => {
  assert.fail('the last generation has no further edit to accept');
});
