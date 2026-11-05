import assert from 'node:assert/strict';
import { captureConsoleLog } from '../../_test_helpers/capture-console-log.mjs';

// Like `runtime_host_external_imports_entry`, but `h.js` reaches the external `ext-async` only
// through `export *` in `barrel.js`. The namespace object of `barrel.js` imports `ext-async` inline,
// and the import tables of the chunk links do not hold that import. So `h.js` must not host the
// runtime: Node exits with code 13 when it does.
const logs = await captureConsoleLog(() => import('./dist/x.js'));
assert.deepEqual(
  logs.filter((log) => log === 'd z' || log.startsWith('e ')),
  ['e start', 'd z', 'e done'],
);
