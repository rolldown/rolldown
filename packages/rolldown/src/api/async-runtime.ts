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
  // Topology must be validated first so a topology violation reports itself
  // rather than a drainer-field error. The drainer budget is config-only, so
  // the shared topology normalization stays field-exact for both reporters.
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
 * cleared and always at least their live gauge in the same snapshot. A legacy
 * Tokio-backed binding never installs the shared scheduler, so its counters
 * stay zero.
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
 * `wasm32-wasip1-threads`, supports `CurrentThread` only. A legacy
 * Tokio-backed binding (`getRuntimeCapabilities().asyncRuntimeBuild ===
 * false`) throws here.
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
  const configureBindingAsyncRuntime = readAsyncRuntimeBindingExport('configureAsyncRuntime');
  Reflect.apply(configureBindingAsyncRuntime, undefined, [options]);
}

/**
 * Return the runtime configuration snapshotted by the binding; this never
 * re-reads environment variables.
 *
 * A legacy Tokio-backed binding predates the `drainLingerUs` field, so its
 * three-field report fails the contract check with reinstall guidance instead
 * of returning a partial snapshot.
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
