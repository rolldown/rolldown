import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

import { isSingleThread, isWasiTest } from '@tests/runtime-flavor';
import { expect, test } from 'vitest';

const testsDir = fileURLToPath(new URL('.', import.meta.url));
const childPath = nodePath.join(testsDir, 'fixtures', 'devtools-close-error', 'child.mjs');

test.skipIf(isWasiTest && isSingleThread)(
  'close preserves closeBundle identity alongside a devtools writer failure',
  { timeout: 30_000 },
  () => {
    const child = spawnSync(process.execPath, [childPath], {
      cwd: testsDir,
      encoding: 'utf8',
      env: { ...process.env },
      timeout: 25_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    const result = JSON.parse(child.stdout.trim().split('\n').at(-1)!);
    expect(result).toEqual({
      closeBundleCalls: 1,
      concurrentPromiseReused: true,
      directBindingFailuresPreserved: true,
      directCloseBundleCalls: 1,
      loneDirectErrorIdentityPreserved: true,
      originalErrorPreserved: true,
      replayedAggregatePreserved: true,
      writerErrorsPreserved: true,
    });
  },
);
