import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { captureConsoleLog } from '../../_test_helpers/capture-console-log.mjs';

// `c` and `h` use helpers. `c` reaches `h` only through `x`, and `x` imports `p` before `h`. So `h`
// hosts the runtime, and the fold adds an import of `h` to `c`. That import must be the last import
// of `c`. As the first import, it runs `h` before `p`. A sort of the imports of `c` puts `h` first:
// `h` runs before `x` in the chunk order, and no module of `c` imports `h.js`, so no source rank
// moves it. With a strict signature, `h.js` is a facade of the chunk that holds `h.js`, and that
// chunk hosts the runtime.
const logs = await captureConsoleLog(() => import('./dist/c.js'));
assert.deepEqual(logs, ['P', 'H h', 'X', 'C c']);

const files = fs.readdirSync(path.join(import.meta.dirname, 'dist'));
assert(!files.some((file) => file.startsWith('rolldown-runtime')), files.join(', '));
