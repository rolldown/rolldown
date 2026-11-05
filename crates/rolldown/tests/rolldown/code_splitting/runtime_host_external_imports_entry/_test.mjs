import assert from 'node:assert/strict';
import { captureConsoleLog } from '../../_test_helpers/capture-console-log.mjs';

// `h.js` and the lazy chunk `d.js` use helpers, and only `h.js` loads `d.js`. `h.js` imports the
// external `ext-async`, and `ext-async` imports the entry `h.js`. The chunk graph does not show
// this cycle, because it passes through the external module. `ext-async` waits with top-level
// await until `d.js` runs, so the cycle is not finished when `h.js` calls `import()`. A new import
// of `h.js` waits for that cycle. As the runtime host, `h.js` gets such an import from `d.js`, and
// Node exits with code 13. So `h.js` must not host the runtime.
const logs = await captureConsoleLog(() => import('./dist/x.js'));
assert.deepEqual(
  logs.filter((log) => log === 'd z' || log.startsWith('e ')),
  ['e start', 'd z', 'e done'],
);
