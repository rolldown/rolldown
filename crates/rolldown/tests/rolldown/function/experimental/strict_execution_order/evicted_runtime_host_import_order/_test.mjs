import assert from 'node:assert';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

// `e0` imports `b` before `a`. The external module behind `b`'s chunk therefore evaluates before
// the one behind `a`'s chunk, although `a`'s chunk hosted the runtime until the order wrappers
// moved it to a chunk of its own.
const logs = await captureConsoleLog(() => import('./dist/e0.js'));

assert.deepStrictEqual(logs, ['ext-b', 'ext-a', 'B', 'A c', 'E0']);
