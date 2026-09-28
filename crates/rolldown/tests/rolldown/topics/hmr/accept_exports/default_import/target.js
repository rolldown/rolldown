import assert from 'node:assert';

export default 0;
export const a = 'a';

globalThis.defaultImportAcceptCount ??= 0;
import.meta.hot.acceptExports(['default'], (mod) => {
  globalThis.defaultImportAcceptCount++;
  assert.strictEqual(mod.default, 1);
});
