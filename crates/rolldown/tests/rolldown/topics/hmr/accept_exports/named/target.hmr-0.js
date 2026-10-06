import assert from 'node:assert';

export const a = 1;
export const b = 'b';

import.meta.hot.acceptExports(['a'], () => {
  assert.fail('the last generation has no further edit to accept');
});
