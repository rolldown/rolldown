import assert from 'node:assert';
import { spawnSync } from 'node:child_process';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

const carrierUrl = new URL('./dist/carrier.js', import.meta.url).href;
const carrier = spawnSync(
  process.execPath,
  [
    '--input-type=module',
    '--eval',
    `const { snapshot } = await import(${JSON.stringify(carrierUrl)}); console.log(snapshot);`,
  ],
  { encoding: 'utf8' },
);

assert.deepStrictEqual(
  { status: carrier.status, stderr: carrier.stderr, stdout: carrier.stdout },
  { status: 0, stderr: '', stdout: 'stale\n' },
);

const logs = await captureConsoleLog(async () => {
  await import('./dist/unused.js');
  await import('./dist/main.js');
});

assert.deepStrictEqual(logs, ['UNUSED', 'E', 'MAIN:ready']);
