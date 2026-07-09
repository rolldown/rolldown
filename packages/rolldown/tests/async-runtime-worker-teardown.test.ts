import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

import { getRuntimeCapabilities } from 'rolldown/experimental';
import { expect, test } from 'vitest';

const caps = getRuntimeCapabilities();
const testsDir = fileURLToPath(new URL('.', import.meta.url));
const childPath = nodePath.join(testsDir, 'fixtures', 'async-runtime-worker-teardown', 'child.mjs');
const requireSharedRuntime = process.env.ROLLDOWN_TEST_REQUIRE_SHARED_ASYNC_RUNTIME === '1';

test.runIf(!caps.wasi && (caps.backend === 'shared' || requireSharedRuntime))(
  'a scheduler waker remains callable after its sole worker environment exits',
  { timeout: 30_000 },
  () => {
    expect(caps.backend).toBe('shared');

    const child = spawnSync(process.execPath, [childPath], {
      cwd: testsDir,
      encoding: 'utf8',
      env: {
        ...process.env,
        ROLLDOWN_RUNTIME: 'single',
      },
      timeout: 25_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    const result = JSON.parse(child.stdout.trim().split('\n').at(-1)!);
    expect(result).toMatchObject({
      backend: 'shared',
      flavor: 'CurrentThread',
      completed: 'completed',
      workerExitedBeforeRelease: true,
    });
  },
);
