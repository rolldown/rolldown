import assert from 'node:assert';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

// m1's bare import of c3 must keep its require trigger inside the order wrapper even though m2
// also reaches c3 through a value import. Source order is [c3, c4, m2, m1, m0].
const logs = await captureConsoleLog(() => import('./dist/main.js'));

assert.deepStrictEqual(logs, ['c3', 'c4', 'm2', 'm1', 'm0']);
