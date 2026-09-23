import * as binding from '../binding.cjs';

/**
 * Executor used by a Rolldown binding built with the shared async runtime.
 *
 * @experimental
 */
export type AsyncRuntimeFlavor = 'CurrentThread' | 'MultiThread';

/**
 * Configuration accepted before the binding starts its first async operation.
 *
 * `MultiThread` promotes a requested single worker to two, applies the
 * platform worker cap, and limits blocking admission to `workerThreads - 1`;
 * `CurrentThread` normalizes both counts to one. Every WebAssembly build is
 * `CurrentThread` only. Without overrides, native builds start from the
 * smaller of physical and process-available CPU counts.
 *
 * @experimental
 */
export interface AsyncRuntimeOptions {
  flavor?: AsyncRuntimeFlavor;
  /** Positive integer worker count, no greater than 256. */
  workerThreads?: number;
  /** Positive integer blocking-task limit, no greater than 256. */
  maxBlockingTasks?: number;
}

/**
 * Executor topology shared by the config and metrics snapshots.
 *
 * @experimental
 */
interface AsyncRuntimeTopology {
  flavor: AsyncRuntimeFlavor;
  workerThreads: number;
  maxBlockingTasks: number;
}

/**
 * Effective, immutable configuration used by the loaded binding.
 *
 * @experimental
 */
export interface AsyncRuntimeConfig extends AsyncRuntimeTopology {
  /**
   * Effective MultiThread drainer idle-linger budget in microseconds
   * (`0` = lingering disabled). Resolved from `ROLLDOWN_DRAIN_LINGER_US` at
   * binding load and reported for introspection parity; not settable
   * through {@link configureAsyncRuntime}.
   */
  drainLingerUs: number;
}

/**
 * Snapshot of shared-runtime scheduler activity plus the executor topology,
 * but not the config-only {@link AsyncRuntimeConfig.drainLingerUs} budget.
 *
 * Event counters are cumulative until {@link resetAsyncRuntimeMetrics}; active
 * fields are live gauges; maximum fields are lifetime high-water marks, never
 * cleared and always at least their live gauge in the same snapshot.
 *
 * @experimental
 */
export interface AsyncRuntimeMetrics extends AsyncRuntimeTopology {
  tasksSpawned: number;
  tasksCompleted: number;
  tasksPanicked: number;
  runnableSchedules: number;
  runnablePolls: number;
  queuedRunnables: number;
  maxQueuedRunnables: number;
  activeRunnables: number;
  maxActiveRunnables: number;
  blockingTasksStarted: number;
  blockingTasksCompleted: number;
  activeBlockingTasks: number;
  maxActiveBlockingTasks: number;
}

/**
 * Configure the shared async runtime before its first async operation.
 *
 * Native bindings support both flavors; every WebAssembly binding, including
 * `wasm32-wasip1-threads`, supports `CurrentThread` only.
 *
 * Configuration is process-wide for the loaded native binding and immutable
 * once the first real runtime generation starts. These are resolved at
 * binding load, before this override:
 *
 * - `ROLLDOWN_RUNTIME=single|current-thread|multi|multi-thread`
 * - `ROLLDOWN_WORKER_THREADS`
 * - `ROLLDOWN_MAX_BLOCKING_THREADS`
 * - `ROLLDOWN_PARK_DEADLINE_MS`
 * - `ROLLDOWN_DRAIN_LINGER_US`
 *
 * Those `ROLLDOWN_*` worker counts are capped at 256; explicit options above
 * their documented limits throw instead of being silently truncated.
 *
 * @experimental
 */
export function configureAsyncRuntime(options: AsyncRuntimeOptions): void {
  binding.configureAsyncRuntime(options);
}

/**
 * Return the runtime configuration snapshotted by the binding; this never
 * re-reads environment variables.
 *
 * @experimental
 */
export function getAsyncRuntimeConfig(): AsyncRuntimeConfig {
  return binding.getAsyncRuntimeConfig();
}

/**
 * Return a point-in-time scheduler metrics snapshot.
 *
 * @experimental
 */
export function getAsyncRuntimeMetrics(): AsyncRuntimeMetrics {
  return binding.getAsyncRuntimeMetrics();
}

/**
 * Reset cumulative event counters.
 *
 * Live gauges and lifetime high-water marks are preserved so concurrent task
 * retirement cannot underflow or corrupt the snapshot.
 *
 * @experimental
 */
export function resetAsyncRuntimeMetrics(): void {
  binding.resetAsyncRuntimeMetrics();
}
