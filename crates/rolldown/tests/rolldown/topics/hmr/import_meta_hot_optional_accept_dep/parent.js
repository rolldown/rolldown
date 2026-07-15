import assert from 'node:assert';
import { count as originalCount } from './child';

globalThis.optionalDepAcceptCount ??= 0;
globalThis.optionalDepParentExecuteCount ??= 0;
globalThis.optionalDepParentExecuteCount++;

// The parent must run exactly once: the accept-dep boundary stops propagation here, so
// editing `child` never re-executes `parent` (it would if the update bubbled past it).
assert.strictEqual(globalThis.optionalDepParentExecuteCount, 1);

let count = originalCount;

// Optional-chained accept-dep. The specifier must be rewritten to `child`'s resolved id,
// otherwise the runtime can't match the accepted dep and this callback never fires.
import.meta.hot?.accept('./child.js', (mod) => {
  count = mod.count;
  globalThis.optionalDepAcceptCount++;
  assert.strictEqual(globalThis.optionalDepAcceptCount, count);
});

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(globalThis.optionalDepAcceptCount, 1);
});
