import assert from 'node:assert';

export const a = 0;

globalThis.sideEffectAcceptCount ??= 0;
import.meta.hot.acceptExports([], (mod) => {
  globalThis.sideEffectAcceptCount++;
  assert.strictEqual(mod.a, 1);
});
