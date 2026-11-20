import assert from 'node:assert/strict';
import { copyFile } from 'node:fs/promises';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

await copyFile(
  new URL('./external.cjs', import.meta.url),
  new URL('./dist/chunks/external.cjs', import.meta.url),
);

const output = await captureConsoleLog(async () => {
  await import('./dist/first/entry.mjs');
  await import('./dist/second/entry.mjs');
});
assert.deepEqual(output, ['external:ready', 'external:ready']);
