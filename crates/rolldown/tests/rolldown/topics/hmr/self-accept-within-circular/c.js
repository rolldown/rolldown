import assert from 'node:assert';
import './b';

export const c = 'c';

assert.strictEqual(c, 'c');

// the pre-edit generation's callback fires with the post-edit exports
import.meta.hot.accept((nextExports) => {
  assert.strictEqual(nextExports.c, 'cc');
});
