import assert from 'node:assert';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

// Entry b's chunk also hosts the shared required-ESM carrier, so entry a's chunk imports it.
// An inline entry trigger would run b's program during a's load; the facade keeps it out.
const logs = await captureConsoleLog(async () => {
  await import('./dist/a.js');
  await import('./dist/b.js');
});

assert.deepStrictEqual(
  logs,
  ['S', 'A', 'B'],
  'CJS entry carrier must not execute entry B while loading entry A',
);
