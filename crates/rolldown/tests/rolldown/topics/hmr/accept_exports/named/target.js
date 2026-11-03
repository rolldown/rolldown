import assert from 'node:assert';

export const a = 0;
export const b = 'b';

globalThis.namedAcceptCount ??= 0;
import.meta.hot.acceptExports(['a'], (mod) => {
  globalThis.namedAcceptCount++;
  assert.strictEqual(mod.a, 1);
});
