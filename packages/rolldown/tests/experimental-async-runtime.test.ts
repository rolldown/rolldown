import { rolldown } from 'rolldown';
import {
  type BindingRuntimeConfig,
  type BindingRuntimeMetrics,
  configureAsyncRuntime,
  getAsyncRuntimeConfig,
  getAsyncRuntimeMetrics,
  resetAsyncRuntimeMetrics,
} from 'rolldown/experimental';
import { describe, expect, test } from 'vitest';

// The four `rolldown/experimental` async-runtime fns are feature-gated (see
// internal-docs/async-runtime/implementation.md, RD-6). They are exported on
// every build, but only the `async-runtime` build honors them. On the default
// `tokio-runtime` build:
//   - `configureAsyncRuntime` throws a feature-disabled error,
//   - `getAsyncRuntimeConfig` reports env/default-derived values,
//   - `getAsyncRuntimeMetrics` returns zeroed counters.
//
// This spec runs against whatever binding is built in the worktree. The
// default-build assertions below MUST execute and pass on the default
// `tokio-runtime` build. The metrics-INCREMENT block is gated behind a
// build-flavor probe so it only asserts on an `async-runtime` artifact.

// The exact feature-disabled message from
// crates/rolldown_binding/src/async_runtime.rs (the `not(feature =
// "async-runtime")` arm). We assert the backtick-wrapped substring.
const FEATURE_DISABLED = 'built without the `async-runtime` feature';

// Probe the build flavor once: on the default build `configureAsyncRuntime`
// throws the feature-disabled error; on an `async-runtime` build it succeeds
// (or throws a different, non-feature-disabled error). `true` => default build.
function detectDefaultBuild(): boolean {
  try {
    // A no-op override: pass nothing to avoid mutating an async-runtime build's
    // real config. On the default build this throws feature-disabled regardless.
    configureAsyncRuntime({});
    return false;
  } catch (error) {
    return String((error as Error)?.message ?? error).includes(FEATURE_DISABLED);
  }
}

const isDefaultBuild = detectDefaultBuild();

// The non-config, non-flavor metrics fields: the pure runtime counters that
// must all be zero on the default build (and that rise after work on an
// async-runtime build).
const COUNTER_FIELDS: Array<keyof BindingRuntimeMetrics> = [
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

describe('experimental async runtime API', () => {
  test('configureAsyncRuntime throws the feature-disabled error on the default build', () => {
    // Guard: only meaningful on the default `tokio-runtime` build. On an
    // async-runtime build configure succeeds, so there is nothing to assert.
    if (!isDefaultBuild) {
      return;
    }
    expect(() => configureAsyncRuntime({ workerThreads: 2 })).toThrow(FEATURE_DISABLED);
  });

  test('getAsyncRuntimeConfig returns MultiThread with positive thread counts', () => {
    const config: BindingRuntimeConfig = getAsyncRuntimeConfig();
    // `BindingRuntimeFlavor` is a napi string_enum; its runtime representation
    // is the string 'MultiThread' (default). See async_runtime.rs.
    expect(config.flavor).toBe('MultiThread');
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
    const metrics: BindingRuntimeMetrics = getAsyncRuntimeMetrics();
    // Flavor/thread fields mirror the config (non-zero); only the counters are 0.
    expect(metrics.flavor).toBe('MultiThread');
    for (const field of COUNTER_FIELDS) {
      expect(metrics[field], `metric ${String(field)}`).toBe(0);
    }
  });

  // The increment path (counters rise after a bundle, reset back to zero) is
  // only exercised on an `async-runtime` build, where the Rolldown scheduler is
  // installed and async binding work is actually counted. On the default
  // `tokio-runtime` build this is a clean skip — the scheduler is Tokio and the
  // counters stay zero, so there is nothing to observe.
  test.skipIf(isDefaultBuild)(
    'metrics rise after a bundle and reset to zero (async-runtime build only)',
    async () => {
      resetAsyncRuntimeMetrics();
      const before = getAsyncRuntimeMetrics();
      for (const field of COUNTER_FIELDS) {
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

      // Reset zeroes the counters again.
      resetAsyncRuntimeMetrics();
      const reset = getAsyncRuntimeMetrics();
      for (const field of COUNTER_FIELDS) {
        expect(reset[field], `post-reset metric ${String(field)}`).toBe(0);
      }
    },
  );
});
