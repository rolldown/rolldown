import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

// Like `runtime_host_cjs_entry_module_exports`, but the CommonJS entry `main.js` rebinds `exports`
// and never names `module`. As the runtime host, `main.js` writes the helper exports through the
// same binding, so they go on the new object. `require()` of `main.js` still returns the old
// object. So `main.js` must not host the runtime.
const require = createRequire(import.meta.url);
assert.deepEqual(require('./dist/main.js'), {});
assert.equal(await globalThis.lazy, 'a,b');
