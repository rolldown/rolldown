import { execFile } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { expect, test } from 'vitest';

test.for([
  { scenario: 'hmr', name: 'HMR executes an unchanged barrel from an initially unloaded chunk' },
  { scenario: 'extra', name: 'HMR preserves initialization through an initially unloaded barrel' },
  { scenario: 'lazy', name: 'the first lazy request executes an initially unloaded barrel' },
])('$name', async ({ scenario }) => {
  const { stdout } = await promisify(execFile)(
    process.execPath,
    [
      '--experimental-vm-modules',
      fileURLToPath(new URL('./fixtures/unloaded-barrel.mjs', import.meta.url)),
      scenario,
    ],
    { timeout: 15_000 },
  );
  expect(JSON.parse(stdout.trim())).toEqual({
    initial: 'rendered:before',
    ...(scenario === 'lazy' ? { lazy: 'rendered:lazy' } : { updated: 'rendered:after' }),
    ...(scenario === 'extra' ? { extra: 'rendered:extra' } : {}),
  });
});
