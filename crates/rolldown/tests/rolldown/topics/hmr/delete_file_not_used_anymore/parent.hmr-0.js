export const childValue = 'not-child';
export const parentValue = 'parent';

import.meta.hot.accept((newMod) => {
  const { childValue, parentValue } = newMod;
  assert.strictEqual(parentValue, 'parent');
  assert.strictEqual(childValue, 'not-child');
});
