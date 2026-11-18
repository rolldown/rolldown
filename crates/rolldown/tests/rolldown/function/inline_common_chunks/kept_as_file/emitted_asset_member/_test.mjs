import assert from 'node:assert';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

// `shared.js` stays a file under `chunks/`, so both entries read the same asset path from it,
// relative to that file.
const logs = await captureConsoleLog(async () => {
  await import('./dist/a.js');
  await import('./dist/b.js');
});

assert.strictEqual(logs.length, 2);
assert.match(logs[0], /^A \.\.\/assets\/logo-[\w-]+\.png$/);
assert.strictEqual(logs[1], logs[0].replace(/^A/, 'B'));
