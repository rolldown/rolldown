import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

// `main.js` and the lazy chunk use helpers, and only `main.js` loads the lazy chunk. But the
// CommonJS entry `main.js` replaces `module.exports`. As the runtime host, `main.js` puts the helper
// exports on the old exports object, so `require()` of `main.js` in the lazy chunk does not return
// them. So `main.js` must not host the runtime.
const require = createRequire(import.meta.url);
assert.deepEqual(require('./dist/main.js'), { name: 'main', esm: 'esm' });
assert.equal(await globalThis.lazy, 'a,b');
