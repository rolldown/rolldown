import assert from 'node:assert';

export default 1;
export const a = 'a';

import.meta.hot.acceptExports(['default'], () => {
  assert.fail('the last generation has no further edit to accept');
});
