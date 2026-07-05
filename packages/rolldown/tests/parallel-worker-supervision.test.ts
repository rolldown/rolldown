import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';

const testsDir = fileURLToPath(new URL('.', import.meta.url));
const childPath = nodePath.join(testsDir, 'fixtures', 'parallel-worker-supervision', 'child.mjs');

test.each([
  ['error', 'delayed parallel-plugin worker fault'],
  ['exit', 'Parallel-plugin worker exited unexpectedly (exit code 23)'],
])(
  'a delayed parallel worker %s is retained through retryable shutdown',
  { timeout: 30_000 },
  (mode, expectedMessage) => {
    const child = spawnSync(process.execPath, [childPath, mode], {
      cwd: testsDir,
      encoding: 'utf8',
      env: { ...process.env },
      timeout: 25_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    const output = JSON.parse(child.stdout.trim().split('\n').at(-1)!);
    expect(output.firstCloseErrors).toContain(expectedMessage);
    expect(output.terminateCallsAfterRetry).toBe(output.terminateCallsAfterFirstClose);
  },
);
