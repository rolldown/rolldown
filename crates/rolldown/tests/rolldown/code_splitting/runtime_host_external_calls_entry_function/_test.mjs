import assert from 'node:assert/strict';
import path from 'node:path';
import { captureConsoleLog } from '../../_test_helpers/capture-console-log.mjs';

// Only `c.js` uses a helper: `__require` in `separator()`. `c.js` imports the external `ext-call`,
// and `ext-call` imports the entry `c.js` and calls `separator()` before the code of `c.js` runs. A
// function declaration exists before the code of its module runs. As the runtime host, `c.js`
// declares `__require` in its code, so `__require` is still `undefined` in that call. So `c.js` must
// not host the runtime, although it is the only consumer: its module is not a CommonJS module.
const logs = await captureConsoleLog(() => import('./dist/c.js'));
assert.deepEqual(logs, [`e ${path.sep}`, 'c']);
