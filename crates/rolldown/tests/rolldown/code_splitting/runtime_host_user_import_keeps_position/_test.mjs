import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { captureConsoleLog } from '../../_test_helpers/capture-console-log.mjs';

// Found by the zarara output fuzz (rolldown/zarara#61). `reader.js` reads the namespace of
// `barrel.js`, so the chunk of `barrel.js` uses helpers. `first.js` loads `second.js` with
// `import()`, so `second.js` gets its own chunk, and that chunk hosts the runtime. Unless the
// signature allows extension, `barrel.js` is a facade that imports `first.js` and then the host,
// for its re-exports. The fold must keep that order. A sort by `exec_order` puts the host first:
// when the chunk order is derived again, the host gets the `exec_order` 0 of the runtime. Then
// `second` logs before `first`.
const logs = await captureConsoleLog(() => import('./dist/barrel.js'));
assert.deepEqual(logs, ['first', 'second', 'barrel']);

const files = fs.readdirSync(path.join(import.meta.dirname, 'dist'));
assert(!files.some((file) => file.startsWith('rolldown-runtime')), files.join(', '));
