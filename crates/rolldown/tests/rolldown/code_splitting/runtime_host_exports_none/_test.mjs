import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

// `main.js` and the lazy chunk use helpers, and only `main.js` loads the lazy chunk. But with
// `exports: 'none'`, the entry `main.js` can export nothing. As the runtime host, `main.js` exports
// the helpers, and the build fails with INVALID_EXPORT_OPTION. So `main.js` must not host the
// runtime.
const require = createRequire(import.meta.url);
assert.deepEqual(require('./dist/main.js'), {});
assert.equal(globalThis.main, 'a');
assert.equal(await globalThis.lazy, 'b');
