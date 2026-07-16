import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

import { getRuntimeCapabilities } from 'rolldown/experimental';
import * as bindingModule from '../src/binding.cjs';
import { expect, test } from 'vitest';

const caps = getRuntimeCapabilities();
const testsDir = fileURLToPath(new URL('.', import.meta.url));
const childPath = nodePath.join(testsDir, 'fixtures', 'async-runtime-worker-teardown', 'child.mjs');
const loaderCancellationChildPath = nodePath.join(
  testsDir,
  'fixtures',
  'async-runtime-worker-teardown',
  'loader-cancellation-child.mjs',
);
const requireSharedRuntime = process.env.ROLLDOWN_TEST_REQUIRE_SHARED_ASYNC_RUNTIME === '1';
// These regressions exercise lifecycle test probes that only the probe-enabled
// binding exports (Native Async Runtime CI job). The regular binding built by
// node-test-ubuntu reports `backend === 'shared'` but lacks the probes, so gate
// each test on the specific probe its child fixture needs instead of letting the
// child throw "built without ... probe". `requireSharedRuntime` bypasses the
// probe gate on purpose: the probe CI lane sets it as a tripwire, and a probe
// binding missing its probes must fail loudly there, never skip.
const asyncRuntimeProbes = bindingModule as unknown as Record<string, unknown>;
const hasWorkerTeardownWakerProbe =
  requireSharedRuntime ||
  typeof asyncRuntimeProbes.__rolldownTestRetainSchedulerWaker === 'function';
const hasSchedulerLifecycleProbes =
  requireSharedRuntime ||
  (typeof asyncRuntimeProbes.__rolldownTestStartAsyncRuntime === 'function' &&
    typeof asyncRuntimeProbes.__rolldownTestStopAsyncRuntime === 'function');

test
  .runIf(
    !caps.wasi &&
      (caps.backend === 'shared' || requireSharedRuntime) &&
      hasWorkerTeardownWakerProbe,
  )
  .each([
    { runtime: 'single', flavor: 'CurrentThread' },
    { runtime: 'multi', flavor: 'MultiThread' },
  ] as const)(
  'a $flavor scheduler waker remains callable after its sole worker environment exits',
  { timeout: 30_000 },
  ({ runtime, flavor }) => {
    expect(caps.backend).toBe('shared');

    const child = spawnSync(process.execPath, [childPath], {
      cwd: testsDir,
      encoding: 'utf8',
      env: {
        ...process.env,
        ROLLDOWN_RUNTIME: runtime,
      },
      timeout: 25_000,
    });

    expect(child.error).toBeUndefined();
    expect(child.signal).toBeNull();
    expect(child.status, child.stderr || child.stdout).toBe(0);
    const result = JSON.parse(child.stdout.trim().split('\n').at(-1)!);
    expect(result).toMatchObject({
      backend: 'shared',
      flavor,
      completed: 'completed',
      workerExitedBeforeRelease: true,
    });
  },
);

test.runIf(
  !caps.wasi && (caps.backend === 'shared' || requireSharedRuntime) && hasSchedulerLifecycleProbes,
)(
  'terminating a worker with a pending loader task does not panic or poison the main realm',
  { timeout: 30_000 },
  () => {
    expect(caps.backend).toBe('shared');

    const child = spawnSync(process.execPath, [loaderCancellationChildPath], {
      cwd: testsDir,
      encoding: 'utf8',
      env: {
        ...process.env,
        ROLLDOWN_RUNTIME: 'multi',
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
      backend: 'shared',
      flavor: 'MultiThread',
      mainBundleGenerations: 2,
      replacementBundleGenerations: 1,
      retiredSchedulerState: {
        activeBlockingTasks: 0,
        activeRunnables: 0,
        queuedRunnables: 0,
      },
      workerExternalSideEffectsEntered: true,
      workerNormalLoadEntered: true,
    });
  },
);
