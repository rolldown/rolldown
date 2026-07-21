import assert from 'node:assert';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

const logs = await captureConsoleLog(async () => {
  await import('./dist/unused.js');
  await import('./dist/main.js');
});

assert.deepStrictEqual(logs, ['UNUSED', 'E', 'MAIN:ready']);
