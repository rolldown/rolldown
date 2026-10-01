import assert from 'node:assert/strict';

const { foo } = await import('./dist/main.js');

assert.deepEqual(foo, {
  chain2: 'modified',
  chain3: 'modified',
  chain4: 'modified',
  chain5: 'modified',
});
