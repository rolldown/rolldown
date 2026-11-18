import assert from 'node:assert';

export const a = 0;
export const b = 'b';

globalThis.promotedAcceptCount ??= 0;
import.meta.hot.acceptExports(['a', 'b'], (mod) => {
  globalThis.promotedAcceptCount++;
  assert.strictEqual(mod.a, 1);
});
