import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import * as bindingModule from '../src/binding.cjs';
import { expect, test } from 'vitest';

const testsDir = fileURLToPath(new URL('.', import.meta.url));
const childPath = path.join(testsDir, 'fixtures', 'async-runtime-submission-failure', 'child.mjs');
const capabilities = bindingModule.getRuntimeCapabilities();
// The child fixture requires the scheduler lifecycle test probes
// (`__rolldownTestStart/StopAsyncRuntime`), which only the probe-enabled
// binding exports (Native Async Runtime CI job). The regular binding built by
// node-test-ubuntu lacks them, so skip there instead of letting the child throw.
const asyncRuntimeProbes = bindingModule as unknown as Record<string, unknown>;
const hasSchedulerLifecycleProbes =
  typeof asyncRuntimeProbes.__rolldownTestStartAsyncRuntime === 'function' &&
  typeof asyncRuntimeProbes.__rolldownTestStopAsyncRuntime === 'function';

test.skipIf(!capabilities.asyncRuntimeBuild || capabilities.wasi || !hasSchedulerLifecycleProbes)(
  'real N-API lifecycle submissions reject and retry without duplicate watcher starts',
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
      watcherBuildEnds: 1,
      watcherBuildStarts: 1,
      watcherRunRejected: true,
    });
  },
);
