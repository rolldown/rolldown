import assert from 'node:assert';

export const a = 0;
export const b = 'b';

globalThis.dynamicImportAcceptCount ??= 0;
import.meta.hot.acceptExports(['a'], (mod) => {
  globalThis.dynamicImportAcceptCount++;
  assert.strictEqual(mod.a, 1);
});
