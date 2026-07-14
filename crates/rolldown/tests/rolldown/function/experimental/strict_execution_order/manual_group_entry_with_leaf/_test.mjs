import assert from 'node:assert';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

// A manual chunk group placing the entry and a deep leaf into the entry chunk displaces the
// leaf behind the common chunk. Source order is [m4, m3, m2, m1, m0].
const logs = await captureConsoleLog(() => import('./dist/main.js'));

assert.deepStrictEqual(logs, ['m4', 'm3', 'm2', 'm1', 'm0']);
