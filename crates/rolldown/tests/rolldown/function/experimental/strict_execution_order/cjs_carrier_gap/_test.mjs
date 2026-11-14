import assert from 'node:assert';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

// Under strictExecutionOrder, a code-split CJS side effect (`c`) reached through a bare-import
// pass-through (`p`) must not run before an eager ESM sensitive module (`e`). Source order is
// [E, C, MAIN]; without the carrier-sensitivity fix strict wrongly emits [C, E, MAIN].
const logs = await captureConsoleLog(() => import('./dist/main.js'));

assert.deepStrictEqual(logs, ['E', 'C', 'MAIN']);
