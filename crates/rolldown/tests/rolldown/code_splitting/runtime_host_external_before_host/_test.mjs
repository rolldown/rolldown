import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import path from 'node:path';
import { captureConsoleLog } from '../../_test_helpers/capture-console-log.mjs';

// `c.js` and `h.js` use helpers, and `c.js` imports `x.js` and then `h.js`. `h.js` imports no
// external module, but `x.js` imports `ext-call`. So `ext-call` runs after `c.js` starts and before
// `h.js` runs. `ext-call` imports the entry `c.js` and calls `separator()`, which reads `__require`.
// As the runtime host, `h.js` declares `__require` in its code, so the call fails. So `h.js` must not
// host the runtime.
// In Node ESM output, a chunk without the runtime creates its own `require`, and then
// `separator()` reads no helper of `h.js`. So the platform is `neutral`: its `__require` calls the
// global `require`, which the test defines.
globalThis.require = createRequire(import.meta.url);
const logs = await captureConsoleLog(() => import('./dist/c.js'));
assert.deepEqual(logs, [`e ${path.sep}`, 'x', 'h q', 'c']);
