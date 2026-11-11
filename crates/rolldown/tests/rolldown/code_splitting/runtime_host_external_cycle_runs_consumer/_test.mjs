import assert from 'node:assert/strict';
import { captureConsoleLog } from '../../_test_helpers/capture-console-log.mjs';

// `h.js` and `d.js` use helpers, and `d.js` imports `h.js`. `h.js` imports the external
// `ext-cycle`, and `ext-cycle` imports the entry `d.js`. The chunk graph does not show this cycle,
// because it passes through the external module. So when `h.js` loads first, the code of `d.js`
// runs before the code of `h.js`, and it calls `__exportAll` for its namespace object. As the
// runtime host, `h.js` declares `__exportAll` in its code, so the call fails. So `h.js` must not
// host the runtime. The failure needs no function declaration.
const logs = await captureConsoleLog(() => import('./dist/h.js'));
assert.deepEqual(logs, ['d z', 'e', 'h q']);
