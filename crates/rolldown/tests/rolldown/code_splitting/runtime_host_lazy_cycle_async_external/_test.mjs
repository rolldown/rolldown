import assert from 'node:assert/strict';
import { captureConsoleLog } from '../../_test_helpers/capture-console-log.mjs';

// The entries and the lazy chunk `d.js` use helpers, and only `h.js` loads `d.js`. `h.js` is in a
// chunk cycle with the group chunk `k`, and `k` imports the external `ext-async`. `ext-async`
// waits with top-level await until `d.js` runs, so the cycle is not finished when `h.js` calls
// `import()`. A new import of `h.js` waits for that cycle. As the runtime host, `h.js` gets such an
// import from `d.js`, and Node exits with code 13. So `h.js` must not host the runtime.
const logs = await captureConsoleLog(() => import('./dist/entry.js'));
assert.deepEqual(
  logs.filter((log) => log === 'd z' || log.startsWith('e ')),
  ['e start', 'd z', 'e done'],
);
