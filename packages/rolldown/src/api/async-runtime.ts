import {
  configureAsyncRuntime as configureBindingAsyncRuntime,
  getAsyncRuntimeConfig as getBindingAsyncRuntimeConfig,
  getAsyncRuntimeMetrics as getBindingAsyncRuntimeMetrics,
  resetAsyncRuntimeMetrics as resetBindingAsyncRuntimeMetrics,
} from '../binding.cjs';
import { BindingMismatchError } from '../utils/binding-mismatch-error';

const ASYNC_RUNTIME_FLAVORS = ['CurrentThread', 'MultiThread'] as const;
const ASYNC_RUNTIME_METRIC_FIELDS = [
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
] as const;

function assertAsyncRuntimeBindingExport(exportName: string, value: unknown): void {
  if (typeof value !== 'function') {
    throw new BindingMismatchError(
      `The loaded Rolldown binding does not expose ${exportName}() as a function. ` +
        'Reinstall Rolldown so the JavaScript package and binding versions match.',
    );
  }
}

class AsyncRuntimeBindingContractError extends BindingMismatchError {
  constructor(exportName: string, detail: string, cause?: unknown) {
    super(
      `The loaded Rolldown binding returned an incompatible ${exportName}() result: ${detail}. ` +
        'Reinstall Rolldown so the JavaScript package and binding versions match.',
      cause === undefined ? undefined : { cause },
    );
    this.name = 'AsyncRuntimeBindingContractError';
  }
}

function readBindingResultObject(exportName: string, value: unknown): Record<PropertyKey, unknown> {
  if (value === null || typeof value !== 'object') {
    throw new AsyncRuntimeBindingContractError(exportName, 'the result is not an object');
  }
  return value as Record<PropertyKey, unknown>;
}

function readBindingResultField(
  exportName: string,
  result: Record<PropertyKey, unknown>,
  field: string,
): unknown {
  try {
    return Reflect.get(result, field, result);
  } catch (error) {
    throw new AsyncRuntimeBindingContractError(
      exportName,
      `the ${field} field could not be read`,
      error,
    );
  }
}

function readAsyncRuntimeFlavor(
  exportName: string,
  result: Record<PropertyKey, unknown>,
): AsyncRuntimeFlavor {
  const flavor = readBindingResultField(exportName, result, 'flavor');
  if (ASYNC_RUNTIME_FLAVORS.some((candidate) => candidate === flavor)) {
    return flavor as AsyncRuntimeFlavor;
  }
  throw new AsyncRuntimeBindingContractError(exportName, 'flavor is not a recognized value');
}

function readAsyncRuntimeInteger(
  exportName: string,
  result: Record<PropertyKey, unknown>,
  field: string,
  minimum: number,
): number {
  const value = readBindingResultField(exportName, result, field);
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < minimum) {
    throw new AsyncRuntimeBindingContractError(
      exportName,
      `${field} must be a safe integer no less than ${minimum}`,
    );
  }
  return value;
}

function normalizeAsyncRuntimeConfig(
  exportName: string,
  result: Record<PropertyKey, unknown>,
): AsyncRuntimeConfig {
  return {
    flavor: readAsyncRuntimeFlavor(exportName, result),
    workerThreads: readAsyncRuntimeInteger(exportName, result, 'workerThreads', 1),
    maxBlockingTasks: readAsyncRuntimeInteger(exportName, result, 'maxBlockingTasks', 1),
  };
}

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
 * MultiThread promotes one worker to two, applies the platform worker cap,
 * and limits blocking admission to `workerThreads - 1`.
 * Without overrides, native shared builds start from the smaller of physical
 * and process-available CPU counts. Native Tokio reports 1.5 times that count
 * (rounded down) with four blocking threads.
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
 * updates. Each maximum is at least its corresponding live gauge in the same
 * snapshot.
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
 * - `ROLLDOWN_PARK_DEADLINE_MS`
 *
 * Native `ROLLDOWN_*` worker counts are capped at 256. Native Tokio
 * blocking-thread counts are capped at 512. Explicit options above their
 * documented limits throw instead of being silently truncated.
 *
 * @experimental
 */
export function configureAsyncRuntime(options: AsyncRuntimeOptions): void {
  assertAsyncRuntimeBindingExport('configureAsyncRuntime', configureBindingAsyncRuntime);
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
  assertAsyncRuntimeBindingExport('getAsyncRuntimeConfig', getBindingAsyncRuntimeConfig);
  const exportName = 'getAsyncRuntimeConfig';
  return normalizeAsyncRuntimeConfig(
    exportName,
    readBindingResultObject(exportName, getBindingAsyncRuntimeConfig()),
  );
}

/**
 * Return a point-in-time scheduler metrics snapshot.
 *
 * @experimental
 */
export function getAsyncRuntimeMetrics(): AsyncRuntimeMetrics {
  assertAsyncRuntimeBindingExport('getAsyncRuntimeMetrics', getBindingAsyncRuntimeMetrics);
  const exportName = 'getAsyncRuntimeMetrics';
  const result = readBindingResultObject(exportName, getBindingAsyncRuntimeMetrics());
  const config = normalizeAsyncRuntimeConfig(exportName, result);
  const metrics = Object.fromEntries(
    ASYNC_RUNTIME_METRIC_FIELDS.map((field) => [
      field,
      readAsyncRuntimeInteger(exportName, result, field, 0),
    ]),
  ) as Pick<AsyncRuntimeMetrics, (typeof ASYNC_RUNTIME_METRIC_FIELDS)[number]>;

  for (const [liveField, maximumField] of [
    ['queuedRunnables', 'maxQueuedRunnables'],
    ['activeRunnables', 'maxActiveRunnables'],
    ['activeBlockingTasks', 'maxActiveBlockingTasks'],
  ] as const) {
    if (metrics[maximumField] < metrics[liveField]) {
      throw new AsyncRuntimeBindingContractError(
        exportName,
        `${maximumField} must be no less than ${liveField}`,
      );
    }
  }

  return { ...config, ...metrics };
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
  assertAsyncRuntimeBindingExport('resetAsyncRuntimeMetrics', resetBindingAsyncRuntimeMetrics);
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
