import assert from 'node:assert';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

const logs = await captureConsoleLog(async () => {
  await import('./dist/e0.js');
  await import('./dist/e1.js');
  await import('./dist/z.js');
});

assert.deepStrictEqual(logs, ['B', 'A c', 'Q', 'E0 ', 'E1', 'Z']);
