import assert from 'node:assert';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

const logs = await captureConsoleLog(async () => {
  await import('./dist/main.js');
  await new Promise((resolve) => setImmediate(resolve));
});

assert.deepStrictEqual(logs, ['read foo', 'read foo']);
