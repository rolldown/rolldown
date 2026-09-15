import assert from 'node:assert';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

// e is an ESM user entry that is also require()d, so it carries an interop wrapper. Entry a's
// chunk imports e's chunk for the binding; the trigger must live in e's facade, not inline.
const logs = await captureConsoleLog(async () => {
  await import('./dist/a.js');
  await import('./dist/b.js');
  await import('./dist/e.js');
});

assert.deepStrictEqual(logs, ['E', 'A', 'B']);
