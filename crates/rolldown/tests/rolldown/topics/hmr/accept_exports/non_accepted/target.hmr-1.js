import assert from 'node:assert';

export const a = 1;
export const b = 'b2';

import.meta.hot.acceptExports(['a'], () => {
  assert.fail('the last generation has no further edit to accept');
});
