import assert from 'node:assert/strict';
import path from 'node:path';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';
import { assertNoStaticImportCycle } from '../../../../_test_helpers/find-static-cycle.mjs';

assertNoStaticImportCycle(path.join(import.meta.dirname, 'dist'));

const logs = await captureConsoleLog(async () => {
  await import('./dist/utility-page.js');
  await import('./dist/app.js');
});
assert.deepEqual(logs, ['false', 'The expected value was received']);
