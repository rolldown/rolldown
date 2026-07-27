import * as binding from '../binding.cjs';
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

function readAsyncRuntimeBindingExport(exportName: string): (...args: unknown[]) => unknown {
  let value: unknown;
  try {
    value = Reflect.get(binding, exportName);
  } catch (error) {
    throw new AsyncRuntimeBindingExportError(exportName, 'the export could not be read', {
      cause: error,
    });
  }
  if (typeof value !== 'function') {
    throw new AsyncRuntimeBindingExportError(exportName, 'the export is not a function');
  }
  return value as (...args: unknown[]) => unknown;
}

function invokeAsyncRuntimeReporter(exportName: string): unknown {
  const reporter = readAsyncRuntimeBindingExport(exportName);
  try {
    return Reflect.apply(reporter, undefined, []);
  } catch (error) {
    throw new AsyncRuntimeBindingContractError(exportName, 'the reporter threw', { cause: error });
  }
}

class AsyncRuntimeBindingExportError extends BindingMismatchError {
  constructor(exportName: string, detail: string, options?: ErrorOptions) {
    super(
      `The loaded Rolldown binding does not expose ${exportName}() as a function: ${detail}. ` +
        'Reinstall Rolldown so the JavaScript package and binding versions match.',
      options,
    );
    this.name = 'AsyncRuntimeBindingExportError';
  }
}

class AsyncRuntimeBindingContractError extends BindingMismatchError {
  constructor(exportName: string, detail: string, options?: ErrorOptions) {
    super(
      `The loaded Rolldown binding returned an incompatible ${exportName}() result: ${detail}. ` +
        'Reinstall Rolldown so the JavaScript package and binding versions match.',
      options,
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
    throw new AsyncRuntimeBindingContractError(exportName, `the ${field} field could not be read`, {
      cause: error,
    });
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

function normalizeAsyncRuntimeTopology(
  exportName: string,
  result: Record<PropertyKey, unknown>,
): AsyncRuntimeTopology {
  const flavor = readAsyncRuntimeFlavor(exportName, result);
  const workerThreads = readAsyncRuntimeInteger(exportName, result, 'workerThreads', 1);
  const maxBlockingTasks = readAsyncRuntimeInteger(exportName, result, 'maxBlockingTasks', 1);
  if (flavor === 'CurrentThread' && (workerThreads !== 1 || maxBlockingTasks !== 1)) {
    throw new AsyncRuntimeBindingContractError(
      exportName,
      'CurrentThread requires workerThreads and maxBlockingTasks to both equal 1',
    );
  }
  return { flavor, workerThreads, maxBlockingTasks };
}

function normalizeAsyncRuntimeConfig(
  exportName: string,
  result: Record<PropertyKey, unknown>,
): AsyncRuntimeConfig {
  // The drainer budget is part of the CONFIG snapshot only; the binding's
  // metrics snapshot deliberately omits it, so the shared topology
  // normalization above stays field-exact for both reporters. Topology is
  // validated first so a topology violation reports itself rather than a
  // drainer-field error.
  const topology = normalizeAsyncRuntimeTopology(exportName, result);
  const drainLingerUs = readAsyncRuntimeInteger(exportName, result, 'drainLingerUs', 0);
  return { ...topology, drainLingerUs };
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
 * and process-available CPU counts.
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
 * Snapshot of shared-runtime scheduler activity.
 *
 * Event counters are cumulative until {@link resetAsyncRuntimeMetrics} is
 * called. Active fields are live gauges. Maximum fields are lifetime
 * high-water marks and are not cleared while live work may still publish
 * updates. Each maximum is at least its corresponding live gauge in the same
 * snapshot.
 *
 * All counters are zero until the shared runtime schedules its first task.
 * A legacy Tokio-backed binding never installs the shared scheduler, so its
 * counters stay zero.
 *
 * The snapshot reports the executor topology but not the config-only
 * {@link AsyncRuntimeConfig.drainLingerUs} budget.
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
 * Every current binding runs the shared runtime: native bindings support
 * both flavors, and every WebAssembly binding, including
 * `wasm32-wasip1-threads`, supports `CurrentThread` only. A legacy
 * Tokio-backed binding (`getRuntimeCapabilities().asyncRuntimeBuild ===
 * false`) throws when this function is called.
 *
 * Configuration is process-wide for the loaded native binding and remains
 * immutable after the first real runtime generation starts. Environment
 * variables are resolved at binding load before this override:
 *
 * - `ROLLDOWN_RUNTIME=single|current-thread|multi|multi-thread`
 * - `ROLLDOWN_WORKER_THREADS`
 * - `ROLLDOWN_MAX_BLOCKING_THREADS`
 * - `ROLLDOWN_PARK_DEADLINE_MS`
 * - `ROLLDOWN_DRAIN_LINGER_US`
 *
 * The drainer linger budget has no option here: `ROLLDOWN_DRAIN_LINGER_US`
 * is resolved once at binding load and reported by
 * {@link getAsyncRuntimeConfig} as `drainLingerUs`.
 *
 * Native `ROLLDOWN_*` worker counts are capped at 256. Explicit options
 * above their documented limits throw instead of being silently truncated.
 *
 * @experimental
 */
export function configureAsyncRuntime(options: AsyncRuntimeOptions): void {
  const configureBindingAsyncRuntime = readAsyncRuntimeBindingExport('configureAsyncRuntime');
  Reflect.apply(configureBindingAsyncRuntime, undefined, [options]);
}

/**
 * Return the effective runtime configuration snapshotted by the binding.
 *
 * This never re-reads environment variables. A legacy Tokio-backed binding
 * predates the `drainLingerUs` field, so its three-field report fails this
 * package's contract check with reinstall guidance instead of returning a
 * partial snapshot.
 *
 * @experimental
 */
export function getAsyncRuntimeConfig(): AsyncRuntimeConfig {
  const exportName = 'getAsyncRuntimeConfig';
  return normalizeAsyncRuntimeConfig(
    exportName,
    readBindingResultObject(exportName, invokeAsyncRuntimeReporter(exportName)),
  );
}

/**
 * Return a point-in-time scheduler metrics snapshot.
 *
 * @experimental
 */
export function getAsyncRuntimeMetrics(): AsyncRuntimeMetrics {
  const exportName = 'getAsyncRuntimeMetrics';
  const result = readBindingResultObject(exportName, invokeAsyncRuntimeReporter(exportName));
  const topology = normalizeAsyncRuntimeTopology(exportName, result);
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

  return { ...topology, ...metrics };
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
  const resetBindingAsyncRuntimeMetrics = readAsyncRuntimeBindingExport('resetAsyncRuntimeMetrics');
  Reflect.apply(resetBindingAsyncRuntimeMetrics, undefined, []);
}

/** @deprecated Use {@link AsyncRuntimeFlavor}. */
export type BindingRuntimeFlavor = AsyncRuntimeFlavor;
/** @deprecated Use {@link AsyncRuntimeOptions}. */
export type BindingRuntimeOptions = AsyncRuntimeOptions;
/** @deprecated Use {@link AsyncRuntimeConfig}. */
export type BindingRuntimeConfig = AsyncRuntimeConfig;
/** @deprecated Use {@link AsyncRuntimeMetrics}. */
export type BindingRuntimeMetrics = AsyncRuntimeMetrics;
