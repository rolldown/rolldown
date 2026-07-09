import {
  configureAsyncRuntime as configureBindingAsyncRuntime,
  getAsyncRuntimeConfig as getBindingAsyncRuntimeConfig,
  getAsyncRuntimeMetrics as getBindingAsyncRuntimeMetrics,
  resetAsyncRuntimeMetrics as resetBindingAsyncRuntimeMetrics,
} from '../binding.cjs';

/**
 * Executor used by a Rolldown binding built with the shared async runtime.
 *
 * @experimental
 */
export type AsyncRuntimeFlavor = 'CurrentThread' | 'MultiThread';

/**
 * Configuration accepted before the binding starts its first async operation.
 *
 * On a native shared-runtime binding, `MultiThread` requires at least two
 * workers and reserves one worker from blocking admission. `CurrentThread`
 * always has one execution lane. Every shared-runtime WebAssembly build is
 * `CurrentThread` only and normalizes both thread-count fields to one.
 *
 * @experimental
 */
export interface AsyncRuntimeOptions {
  flavor?: AsyncRuntimeFlavor;
  /** Positive integer worker count, no greater than `2^32 - 1`. */
  workerThreads?: number;
  /** Positive integer blocking-task limit, no greater than `2^32 - 1`. */
  maxBlockingTasks?: number;
}

/**
 * Effective, immutable configuration used by the loaded binding.
 *
 * @experimental
 */
export interface AsyncRuntimeConfig {
  flavor: AsyncRuntimeFlavor;
  workerThreads: number;
  maxBlockingTasks: number;
}

/**
 * Snapshot of shared-runtime scheduler activity.
 *
 * Event counters are cumulative until {@link resetAsyncRuntimeMetrics} is
 * called. Active fields are live gauges. Maximum fields are lifetime
 * high-water marks and are not cleared while live work may still publish
 * updates.
 *
 * All counters are zero on a binding built with the default Tokio backend.
 *
 * @experimental
 */
export interface AsyncRuntimeMetrics extends AsyncRuntimeConfig {
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
 * Use `getRuntimeCapabilities().asyncRuntimeBuild` to detect support. The
 * default native npm binding and the published threaded-WASI binding both use
 * Tokio and throw when this function is called. A custom shared-runtime native
 * binding supports both flavors. A custom shared-runtime WebAssembly binding,
 * including `wasm32-wasip1-threads`, supports `CurrentThread` only.
 *
 * Configuration is process-wide for the loaded native binding and remains
 * immutable after the first real runtime generation starts. Environment
 * variables are resolved at binding load before this override:
 *
 * - `ROLLDOWN_RUNTIME=single|current-thread|multi|multi-thread`
 * - `ROLLDOWN_WORKER_THREADS`
 * - `ROLLDOWN_MAX_BLOCKING_THREADS`
 *
 * @experimental
 */
export function configureAsyncRuntime(options: AsyncRuntimeOptions): void {
  configureBindingAsyncRuntime(options);
}

/**
 * Return the effective runtime configuration snapshotted by the binding.
 *
 * This never re-reads environment variables. On a Tokio binding it reports
 * the Tokio-derived configuration even though {@link configureAsyncRuntime}
 * is unavailable. On the published threaded-WASI binding, the thread counts
 * report the generated loader's effective emnapi pool size, capped at 1024.
 *
 * @experimental
 */
export function getAsyncRuntimeConfig(): AsyncRuntimeConfig {
  return getBindingAsyncRuntimeConfig();
}

/**
 * Return a point-in-time scheduler metrics snapshot.
 *
 * @experimental
 */
export function getAsyncRuntimeMetrics(): AsyncRuntimeMetrics {
  return getBindingAsyncRuntimeMetrics();
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
  resetBindingAsyncRuntimeMetrics();
}

/** @deprecated Use {@link AsyncRuntimeFlavor}. */
export type BindingRuntimeFlavor = AsyncRuntimeFlavor;
/** @deprecated Use {@link AsyncRuntimeOptions}. */
export type BindingRuntimeOptions = AsyncRuntimeOptions;
/** @deprecated Use {@link AsyncRuntimeConfig}. */
export type BindingRuntimeConfig = AsyncRuntimeConfig;
/** @deprecated Use {@link AsyncRuntimeMetrics}. */
export type BindingRuntimeMetrics = AsyncRuntimeMetrics;
