import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

import { isWasiTest } from '@tests/runtime-flavor';
import { expect, test } from 'vitest';

// Native magic-string sourcemaps go to the sourcemap thread instead of being
// generated on the JS thread.

const testsDir = fileURLToPath(new URL('.', import.meta.url));
const childPath = nodePath.join(testsDir, 'fixtures', 'sourcemap-offload-thread', 'child.mjs');

function runChild() {
  const child = spawnSync(process.execPath, [childPath], {
    cwd: testsDir,
    encoding: 'utf8',
    env: process.env,
    timeout: 25_000,
  });

  expect(child.error).toBeUndefined();
  expect(child.signal).toBeNull();
  expect(child.status, child.stderr || child.stdout).toBe(0);
  return JSON.parse(child.stdout.trim().split('\n').at(-1)!);
}

test.skipIf(isWasiTest)(
  'native magic-string sourcemaps stay on the sourcemap thread',
  { timeout: 60_000 },
  () => {
    const result = runChild();

    expect(result.inline).toBe(0);
    expect(result.offloaded).toBe(result.expected);
  },
);
