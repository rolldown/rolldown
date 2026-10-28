import assert from 'node:assert';

export const a = 0;
export const b = 'b';

globalThis.reexportNamedAcceptCount ??= 0;
import.meta.hot?.acceptExports(['a'], (mod) => {
  globalThis.reexportNamedAcceptCount++;
  assert.strictEqual(mod.a, 1);
});
