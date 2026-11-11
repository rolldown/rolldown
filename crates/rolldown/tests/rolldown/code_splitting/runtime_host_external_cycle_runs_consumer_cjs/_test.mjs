import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { captureConsoleLog } from '../../_test_helpers/capture-console-log.mjs';

// Like `runtime_host_external_cycle_runs_consumer`, in CJS output. A chunk assigns its exports at
// its end. So when `ext-cycle` requires `d.js` and `d.js` requires `h.js`, `d.js` gets the
// incomplete exports of `h.js`. As the runtime host, `h.js` exports `__exportAll` to `d.js`, so
// `d.js` gets `undefined`. So `h.js` must not host the runtime.
const require = createRequire(import.meta.url);
const logs = await captureConsoleLog(() => require('./dist/h.js'));
assert.deepEqual(logs, ['d z', 'e', 'h q']);
