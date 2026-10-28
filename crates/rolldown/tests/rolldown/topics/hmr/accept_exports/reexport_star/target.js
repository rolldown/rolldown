import assert from 'node:assert';

export const a = 0;
export const b = 'b';

globalThis.reexportStarAcceptCount ??= 0;
import.meta.hot.acceptExports(['a'], (mod) => {
  globalThis.reexportStarAcceptCount++;
  assert.strictEqual(mod.a, 1);
});
