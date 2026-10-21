import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

import { getRuntimeSupport } from 'rolldown/experimental';
import * as bindingModule from '../src/binding.cjs';
import { expect, test } from 'vitest';

// Parallel plugins are supported only by the native binary.
const native = getRuntimeSupport().parallelPlugins;
const testsDir = fileURLToPath(new URL('.', import.meta.url));
const loaderCancellationChildPath = nodePath.join(
  testsDir,
  'fixtures',
  'async-runtime-worker-teardown',
  'loader-cancellation-child.mjs',
);
const requireSharedRuntime = process.env.ROLLDOWN_TEST_REQUIRE_SHARED_ASYNC_RUNTIME === '1';
// Only the probe binding (`just build-rolldown-async-runtime`) exports these
// probes, so gate on them. The probe CI lane sets `requireSharedRuntime` so a
// binding missing them fails, not skips.
const asyncRuntimeProbes = bindingModule as unknown as Record<string, unknown>;
const hasSchedulerLifecycleProbes =
  requireSharedRuntime ||
  (typeof asyncRuntimeProbes.__rolldownTestStartAsyncRuntime === 'function' &&
    typeof asyncRuntimeProbes.__rolldownTestStopAsyncRuntime === 'function');

test.runIf((native || requireSharedRuntime) && hasSchedulerLifecycleProbes)(
  'terminating a worker with a pending loader task does not panic or poison the main realm',
  { timeout: 30_000 },
  () => {
    const child = spawnSync(process.execPath, [loaderCancellationChildPath], {
      cwd: testsDir,
      encoding: 'utf8',
      env: {
        ...process.env,
        RUST_BACKTRACE: '0',
      },
      timeout: 25_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    expect(child.stderr).not.toContain('Rolldown panicked');
    const result = JSON.parse(child.stdout.trim().split('\n').at(-1)!);
    expect(result).toMatchObject({
      mainBundleGenerations: 2,
      replacementBundleGenerations: 1,
      workerExternalSideEffectsEntered: true,
      workerNormalLoadEntered: true,
    });
  },
);
