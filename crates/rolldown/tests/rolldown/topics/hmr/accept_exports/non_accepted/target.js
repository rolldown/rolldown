import assert from 'node:assert';

export const a = 0;
export const b = 'b';

globalThis.nonAcceptedAcceptCount ??= 0;
import.meta.hot.acceptExports(['a'], (mod) => {
  globalThis.nonAcceptedAcceptCount++;
  assert.strictEqual(mod.a, 1);
});
