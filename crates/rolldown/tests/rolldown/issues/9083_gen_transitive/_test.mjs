import assert from 'node:assert';
import { syncValue } from './dist/main.js';

assert.strictEqual(syncValue, 'sync');
// deep.js sets __deepReady after four awaits.  Without the fix,
// generate_transitive_esm_init emits init_deep() without await; the
// four-await delay means __deepReady is still undefined when _test.mjs
// runs (barrel's TLA resolves before deep.js finishes).
assert.strictEqual(globalThis.__deepReady, true, 'deep.js TLA not properly awaited in barrel init');
