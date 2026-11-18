import assert from 'node:assert';

export const a = 0;
export const b = 'b';

globalThis.multiRecordAcceptCount ??= 0;
import.meta.hot.acceptExports(['a'], (mod) => {
  globalThis.multiRecordAcceptCount++;
  assert.strictEqual(mod.a, 1);
});
