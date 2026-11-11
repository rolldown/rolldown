import assert from 'node:assert/strict';
import { captureConsoleLog } from '../../_test_helpers/capture-console-log.mjs';

// `h.js` and `d.js` use helpers. `h.js` loads `d.js` with `import()` and then throws. `d.js` does
// not import `h.js`, so it still runs. As the runtime host, `h.js` gives `d.js` an import of
// `h.js`. Then `d.js` fails with the error of `h.js`. So `h.js` must not host the runtime. Rollup
// keeps the source behavior here: it uses no shared helper, so `d.js` imports no chunk.
const logs = await captureConsoleLog(async () => {
  await assert.rejects(import('./dist/h.js'), { message: 'host failed' });
  assert.equal(await globalThis.lazy, 'loaded');
});
assert.deepEqual(logs, ['h q', 'd z']);
