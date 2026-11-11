import assert from 'node:assert/strict';
import path from 'node:path';
import { captureConsoleLog } from '../../_test_helpers/capture-console-log.mjs';

// `c.js` and `h.js` use helpers, and `c.js` imports `x.js` and then `h.js`. `h.js` imports no
// external module, but `x.js` imports `ext-call`. So `ext-call` runs after `c.js` starts and before
// `h.js` runs. `ext-call` imports the entry `c.js` and calls `separator()`, which reads `__require`.
// As the runtime host, `h.js` declares `__require` in its code, so the call fails. So `h.js` must not
// host the runtime.
const logs = await captureConsoleLog(() => import('./dist/c.js'));
assert.deepEqual(logs, [`e ${path.sep}`, 'x', 'h q', 'c']);
