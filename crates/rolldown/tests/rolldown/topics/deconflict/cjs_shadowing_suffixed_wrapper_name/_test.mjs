import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';

// Regression test for issue #10792: an author-local named `require_dup$1` inside a CJS-wrapped
// module shadowed the chunk-root wrapper that deconfliction had renamed to `require_dup$1`, so the
// bundle threw `ReferenceError: Cannot access 'require_dup$1' before initialization` at
// evaluation. The wrapper for `./b/dup.cjs` was dropped from the output entirely along the way.
const require = createRequire(import.meta.url);
const mod =
  globalThis.__configName === 'cjs'
    ? require('./dist/main.js')
    : (await import('./dist/main.js')).default;

assert.deepEqual(mod, { a: 'a', b: 'b' });
