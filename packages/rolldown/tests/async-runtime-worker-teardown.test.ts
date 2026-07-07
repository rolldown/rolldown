import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

import { getRuntimeCapabilities } from 'rolldown/experimental';
import { expect, test } from 'vitest';

const caps = getRuntimeCapabilities();
const testsDir = fileURLToPath(new URL('.', import.meta.url));
const childPath = nodePath.join(testsDir, 'fixtures', 'async-runtime-worker-teardown', 'child.mjs');

test.runIf(caps.backend === 'shared')(
  'a scheduler waker remains callable after its sole worker environment exits',
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
    expect(result).toMatchObject({
      backend: 'shared',
      completed: 'completed',
      workerExitedBeforeRelease: true,
    });
  },
);
