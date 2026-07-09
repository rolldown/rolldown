import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { getRuntimeCapabilities } from '../src/binding.cjs';
import { expect, test } from 'vitest';

const testsDir = fileURLToPath(new URL('.', import.meta.url));
const childPath = path.join(testsDir, 'fixtures', 'async-runtime-submission-failure', 'child.mjs');
const capabilities = getRuntimeCapabilities();

test.skipIf(!capabilities.asyncRuntimeBuild || capabilities.wasi)(
  'real N-API close submission rejection is retryable',
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
    expect(JSON.parse(child.stdout.trim().split('\n').at(-1)!)).toEqual({
      closeBundleCalls: 1,
      replayedTerminalError: true,
      submissionRejected: true,
    });
  },
);
