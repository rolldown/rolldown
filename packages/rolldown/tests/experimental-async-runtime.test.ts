import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

import { rolldown } from 'rolldown';
import {
  type AsyncRuntimeConfig,
  type AsyncRuntimeMetrics,
  configureAsyncRuntime,
  getAsyncRuntimeConfig,
  getAsyncRuntimeMetrics,
  getRuntimeCapabilities,
  resetAsyncRuntimeMetrics,
} from 'rolldown/experimental';
import { describe, expect, test } from 'vitest';

// The four `rolldown/experimental` async-runtime fns are feature-gated. They
// are exported on
// every build, but only the `async-runtime` build honors them. On the default
// `tokio-runtime` build:
//   - `configureAsyncRuntime` throws a feature-disabled error,
//   - `getAsyncRuntimeConfig` reports env/default-derived values,
//   - `getAsyncRuntimeMetrics` returns zeroed counters.
//
// This spec runs against whatever binding is built in the worktree. The
// default-build assertions below MUST execute and pass on the default
// `tokio-runtime` build. The metrics-INCREMENT block is gated behind the
// artifact's own capability report so it only asserts on an `async-runtime`
// artifact.

// The exact feature-disabled message from
// crates/rolldown_binding/src/async_runtime.rs (the `not(feature =
// "async-runtime")` arm). We assert the backtick-wrapped substring.
const FEATURE_DISABLED = 'built without the `async-runtime` feature';

// The build flavor comes from the artifact's own capability report; no
// configure-probe against the error message. `true` => default tokio build.
const capabilities = getRuntimeCapabilities();
const isDefaultBuild = !capabilities.asyncRuntimeBuild;
const testsDir = fileURLToPath(new URL('.', import.meta.url));
const flavorSwitchChildPath = nodePath.join(
  testsDir,
  'fixtures',
  'async-runtime-flavor-switch',
  'child.mjs',
);
const configValidationChildPath = nodePath.join(
  testsDir,
  'fixtures',
  'async-runtime-config-validation',
  'child.mjs',
);

// The non-config, non-flavor metrics fields: the pure runtime counters that
// must all be zero on the default build (and that rise after work on an
// async-runtime build).
type NumericMetricField = Exclude<keyof AsyncRuntimeMetrics, 'flavor'>;

const COUNTER_FIELDS: NumericMetricField[] = [
  'tasksSpawned',
  'tasksCompleted',
  'tasksPanicked',
  'runnableSchedules',
  'runnablePolls',
  'queuedRunnables',
  'maxQueuedRunnables',
  'activeRunnables',
  'maxActiveRunnables',
  'blockingTasksStarted',
  'blockingTasksCompleted',
  'activeBlockingTasks',
  'maxActiveBlockingTasks',
];

const RESETTABLE_EVENT_FIELDS: NumericMetricField[] = [
  'tasksSpawned',
  'tasksCompleted',
  'tasksPanicked',
  'runnableSchedules',
  'runnablePolls',
  'blockingTasksStarted',
  'blockingTasksCompleted',
];

const HIGH_WATER_FIELDS = [
  'maxQueuedRunnables',
  'maxActiveRunnables',
  'maxActiveBlockingTasks',
] as const satisfies readonly NumericMetricField[];

const LIVE_GAUGE_HIGH_WATER_FIELDS = [
  ['queuedRunnables', 'maxQueuedRunnables'],
  ['activeRunnables', 'maxActiveRunnables'],
  ['activeBlockingTasks', 'maxActiveBlockingTasks'],
] as const satisfies readonly (readonly [NumericMetricField, NumericMetricField])[];

describe('experimental async runtime API', () => {
  test('configureAsyncRuntime throws the feature-disabled error on the default build', () => {
    // Guard: only meaningful on the default `tokio-runtime` build. On an
    // async-runtime build configure succeeds, so there is nothing to assert.
    if (!isDefaultBuild) {
      return;
    }
    expect(() => configureAsyncRuntime({ workerThreads: 2 })).toThrow(FEATURE_DISABLED);
  });

  test.runIf(capabilities.asyncRuntimeBuild && !capabilities.wasi)(
    'native pre-first-use configuration rejects unsafe numbers atomically and retains hosts across a flavor switch',
    { timeout: 30_000 },
    () => {
      const child = spawnSync(process.execPath, [flavorSwitchChildPath], {
        cwd: testsDir,
        encoding: 'utf8',
        env: {
          ...process.env,
          ROLLDOWN_RUNTIME: 'multi',
        },
        timeout: 25_000,
      });

      expect(child.error).toBeUndefined();
      expect(child.signal).toBeNull();
      expect(child.status, child.stderr || child.stdout).toBe(0);
      const result = JSON.parse(child.stdout.trim().split('\n').at(-1)!);
      expect(result).toMatchObject({
        flavor: 'CurrentThread',
        invalidConfigurationsRejected: 12,
        scanSettled: true,
        buildSettled: true,
      });
    },
  );

  test.runIf(!isDefaultBuild)(
    'configureAsyncRuntime rejects malformed thread counts before scheduler setup',
    { timeout: 30_000 },
    () => {
      const child = spawnSync(process.execPath, [configValidationChildPath], {
        cwd: testsDir,
        encoding: 'utf8',
        env: { ...process.env },
        timeout: 25_000,
      });

      expect(child.error).toBeUndefined();
      expect(child.signal).toBeNull();
      expect(child.status, child.stderr || child.stdout).toBe(0);
      const result = JSON.parse(child.stdout.trim().split('\n').at(-1)!);
      expect(result.workerErrors).toHaveLength(6);
      for (const message of result.workerErrors) {
        expect(message).toContain('`workerThreads` must be a positive integer');
      }
      expect(result.blockingError).toContain('`maxBlockingTasks` must be a positive integer');
      expect(result.config.workerThreads).toBeGreaterThan(0);
      expect(result.config.maxBlockingTasks).toBe(1);
    },
  );

  test('getAsyncRuntimeConfig returns the build flavor with positive thread counts', () => {
    const config: AsyncRuntimeConfig = getAsyncRuntimeConfig();
    // `BindingRuntimeFlavor` is a napi string_enum; its runtime representation
    // is 'MultiThread' or 'CurrentThread'. Tokio builds report MultiThread.
    // Shared native builds report the configured flavor, while every shared
    // WebAssembly build is CurrentThread-only.
    if (isDefaultBuild) {
      expect(config.flavor).toBe(capabilities.flavor);
    } else if (capabilities.wasi) {
      expect(config).toMatchObject({
        flavor: 'CurrentThread',
        maxBlockingTasks: 1,
        workerThreads: 1,
      });
    } else {
      expect(['MultiThread', 'CurrentThread']).toContain(config.flavor);
    }
    // env/default-derived — assert positivity, never a host-specific count.
    expect(config.workerThreads).toBeGreaterThan(0);
    expect(Number.isInteger(config.workerThreads)).toBe(true);
    expect(config.maxBlockingTasks).toBeGreaterThan(0);
    expect(Number.isInteger(config.maxBlockingTasks)).toBe(true);
  });

  test('getAsyncRuntimeMetrics reports all-zero counters on the default build', () => {
    // Guard: zeroed counters are the documented default-build contract. On an
    // async-runtime build prior tests may have already spawned tasks.
    if (!isDefaultBuild) {
      return;
    }
    const metrics: AsyncRuntimeMetrics = getAsyncRuntimeMetrics();
    // Flavor/thread fields mirror the config (non-zero); only the counters are 0.
    expect(metrics.flavor).toBe('MultiThread');
    for (const field of COUNTER_FIELDS) {
      expect(metrics[field], `metric ${String(field)}`).toBe(0);
    }
  });

  // The increment path (event counters rise after a bundle, then reset) is
  // only exercised on an `async-runtime` build, where the Rolldown scheduler is
  // installed and async binding work is actually counted. On the default
  // `tokio-runtime` build this is a clean skip — the scheduler is Tokio and the
  // counters stay zero, so there is nothing to observe.
  test.skipIf(isDefaultBuild)(
    'event metrics reset without corrupting live gauges (async-runtime build only)',
    async () => {
      resetAsyncRuntimeMetrics();
      const before = getAsyncRuntimeMetrics();
      for (const field of RESETTABLE_EVENT_FIELDS) {
        expect(before[field], `reset metric ${String(field)}`).toBe(0);
      }

      const b = await rolldown({
        input: 'virtual:main',
        plugins: [
          {
            name: 'async-runtime-smoke',
            resolveId(id) {
              if (id === 'virtual:main') return '\0' + id;
            },
            load(id) {
              if (id === '\0virtual:main') {
                return `export const answer = 42;\nconsole.log(answer);`;
              }
            },
          },
        ],
      });
      await b.generate({ format: 'esm' });
      await b.close();

      const after = getAsyncRuntimeMetrics();
      // A real bundle drives the scheduler: at least one task is spawned.
      expect(after.tasksSpawned).toBeGreaterThan(before.tasksSpawned);

      // Reset clears cumulative events but preserves live gauges and lifetime
      // high-water marks. N-API may resolve the close promise from inside the
      // runnable's final poll, before that poll's active guard retires.
      resetAsyncRuntimeMetrics();
      const reset = getAsyncRuntimeMetrics();
      for (const field of RESETTABLE_EVENT_FIELDS) {
        expect(
          Number.isSafeInteger(reset[field]),
          `post-reset metric ${String(field)} remains a safe integer`,
        ).toBe(true);
        expect(
          reset[field],
          `post-reset metric ${String(field)} remains non-negative`,
        ).toBeGreaterThanOrEqual(0);
      }
      for (const [liveField, highWaterField] of LIVE_GAUGE_HIGH_WATER_FIELDS) {
        expect(
          reset[liveField],
          `live metric ${String(liveField)} remains bounded after reset`,
        ).toBeLessThanOrEqual(reset[highWaterField]);
      }
      for (const field of HIGH_WATER_FIELDS) {
        expect(reset[field], `preserved metric ${String(field)}`).toBeGreaterThanOrEqual(
          after[field],
        );
      }

      // The final scheduler guard must still retire normally after reset. A
      // wrapped unsigned gauge would remain very large instead of reaching 0.
      await expect
        .poll(
          () => {
            const current = getAsyncRuntimeMetrics();
            return LIVE_GAUGE_HIGH_WATER_FIELDS.map(([field]) => current[field]);
          },
          { timeout: 5_000 },
        )
        .toEqual(LIVE_GAUGE_HIGH_WATER_FIELDS.map(() => 0));

      // Once all live guards have retired, a second reset has no concurrent
      // publisher and must clear every cumulative event exactly.
      resetAsyncRuntimeMetrics();
      const settled = getAsyncRuntimeMetrics();
      for (const field of RESETTABLE_EVENT_FIELDS) {
        expect(settled[field], `quiescent reset metric ${String(field)}`).toBe(0);
      }
    },
  );
});
