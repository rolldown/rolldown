import assert from 'node:assert';

import('./mid.js').then(async (mid) => {
  assert.strictEqual(mid.midVal, 'mid:shared');
  const leaf = await mid.getLeaf();
  assert.strictEqual(leaf.leafVal, 'leaf:shared');
});
