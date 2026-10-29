import assert from 'node:assert';

import('./target.js', {}).then((target) => {
  assert.strictEqual(target.value, 42);
});
