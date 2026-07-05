import { type BindingWatcherEvent, BindingWatcher } from '../../binding.cjs';
import { LOG_LEVEL_WARN } from '../../log/logging';
import { logMultipleWatcherOption } from '../../log/logs';
import { aggregateBindingErrorsIntoJsError } from '../../utils/error';
import type { WatchOptions } from '../../options/watch-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import {
  acquireRuntimeLease,
  CloseCoordinator,
  type CloseAttemptResult,
  type RuntimeLease,
} from '../../runtime-lifecycle';
import {
  type BundlerOptionWithStopWorker,
  createBundlerOptions,
} from '../../utils/create-bundler-option';
import {
  createCleanupFailureError,
  getRetryableCleanup,
  hasRetryableCleanupOwnership,
  retryCleanupFromError,
  runRetryableCleanup,
  trackRetryableCleanupOwnership,
  type RetryableCleanup,
} from '../../utils/retryable-cleanup';
import { arraify } from '../../utils/misc';
import type { WatcherEmitter } from './watch-emitter';

function createEventCallback(
  emitter: WatcherEmitter,
  onNativeClose: () => void,
): (event: BindingWatcherEvent) => Promise<void> {
  return async (event: BindingWatcherEvent) => {
    switch (event.eventKind()) {
      case 'event': {
        const code = event.bundleEventKind();
        if (code === 'BUNDLE_END') {
          const { duration, output, result } = event.bundleEndData();
          await emitter.emit('event', {
            code: 'BUNDLE_END',
            duration,
            output: [output],
            result,
          });
        } else if (code === 'ERROR') {
          const data = event.bundleErrorData();
          await emitter.emit('event', {
            code: 'ERROR',
            error: aggregateBindingErrorsIntoJsError(data.error),
            result: data.result,
          });
        } else {
          await emitter.emit('event', { code: code as 'START' | 'BUNDLE_START' | 'END' });
        }
        break;
      }
      case 'change': {
        const { path, kind } = event.watchChangeData();
        await emitter.emit('change', path, {
          event: kind as 'create' | 'update' | 'delete',
        });
        break;
      }
      case 'restart':
        await emitter.emit('restart');
        break;
      case 'close':
        // The native coordinator awaits this callback. Dispatching listeners
        // here would make a close listener that calls `watcher.close()`
        // self-await the coordinator. Start/observe the JS close lifecycle
        // without awaiting listener dispatch; the public close promise emits
        // after native cleanup and worker termination finish.
        onNativeClose();
        break;
    }
  };
}

class Watcher {
  closed: boolean;
  inner: BindingWatcher;
  emitter: WatcherEmitter;
  runtimeLease: RuntimeLease;
  stopWorkers: ((() => Promise<void>) | undefined)[];
  scheduledRun: ReturnType<typeof setTimeout> | undefined;
  runFailure: unknown;
  nativeClosePromise: Promise<void> | undefined;
  closeEventPromise: Promise<void> | undefined;
  closeCoordinator = new CloseCoordinator(
    'Watcher native close, parallel-plugin worker shutdown, close listener, or runtime release failed',
  );

  constructor(
    emitter: WatcherEmitter,
    inner: BindingWatcher,
    runtimeLease: RuntimeLease,
    stopWorkers: ((() => Promise<void>) | undefined)[],
  ) {
    this.closed = false;
    this.inner = inner;
    this.emitter = emitter;
    this.runtimeLease = runtimeLease;
    this.stopWorkers = stopWorkers;
  }

  start(): void {
    // Defer so watch() returns the emitter before the first build,
    // giving the caller a chance to attach .on() handlers.
    // A timer is a host turn in both browsers and Node.js.
    this.scheduledRun = globalThis.setTimeout(() => {
      this.scheduledRun = undefined;
      if (this.closed) return;
      void this.run().catch((error) => {
        this.runFailure ??= error;
        // Preserve the failure for a later public close while ensuring the
        // native watcher, workers, and runtime lease are not abandoned.
        void this.close().catch(() => {});
      });
    }, 0);
  }

  close(): Promise<void> {
    return this.closeCoordinator.close(() => this.closeLifecycle());
  }

  onNativeClose(): void {
    // Native close can be observed without a public caller (for example if
    // the coordinator exits independently). Preserve the memoized rejection
    // for a later `close()` call while avoiding an unhandled rejection.
    void this.close().catch(() => {});
  }

  private async closeLifecycle(): Promise<CloseAttemptResult> {
    const result = await this.closeOwnedResources();

    try {
      this.closeEventPromise ??= this.dispatchCloseEvent();
      await this.closeEventPromise;
    } catch (error) {
      result.errors.push(error);
    }

    try {
      this.runtimeLease.release();
    } catch (error) {
      result.errors.push(error);
      result.retryable = true;
    }

    return result;
  }

  async cleanupAfterSetupFailure(): Promise<CloseAttemptResult> {
    const result = await this.closeOwnedResources();
    try {
      this.runtimeLease.release();
    } catch (error) {
      result.errors.push(error);
      result.retryable = true;
    }
    return result;
  }

  private async closeOwnedResources(): Promise<CloseAttemptResult> {
    this.closed = true;
    const errors: unknown[] = this.runFailure === undefined ? [] : [this.runFailure];
    this.cancelScheduledRun(errors);
    this.nativeClosePromise ??= (async () => this.inner.close())();

    let retryable = false;
    try {
      await this.nativeClosePromise;
    } catch (error) {
      errors.push(error);
    }

    const stopWorkers = this.stopWorkers;
    const workerResults = await Promise.allSettled(stopWorkers.map(async (stop) => stop?.()));
    this.stopWorkers = stopWorkers.filter((_, index) => workerResults[index].status === 'rejected');
    for (const result of workerResults) {
      if (result.status === 'rejected') {
        errors.push(result.reason);
        retryable = true;
      }
    }

    return { errors, retryable };
  }

  private cancelScheduledRun(errors: unknown[]): void {
    if (this.scheduledRun === undefined) return;
    const scheduledRun = this.scheduledRun;
    this.scheduledRun = undefined;
    try {
      globalThis.clearTimeout(scheduledRun);
    } catch (error) {
      errors.push(error);
    }
  }

  private async dispatchCloseEvent(): Promise<void> {
    await this.emitter.emitClose(this.nativeClosePromise ?? Promise.resolve());
  }

  private async run(): Promise<void> {
    await this.inner.run();
    // No `.await`: Create pending Promise to keep Node.js event loop alive
    this.inner.waitForClose();
  }
}

export async function createWatcher(
  emitter: WatcherEmitter,
  input: WatchOptions | WatchOptions[],
): Promise<void> {
  const options = arraify(input);
  const bundlerOptionResults = await Promise.allSettled(
    options
      .map((option) =>
        arraify(option.output || {}).map(async (output) => {
          const inputOptions = await PluginDriver.callOptionsHook(option, true);
          return createBundlerOptions(inputOptions, output, true);
        }),
      )
      .flat(),
  );
  const bundlerOptions: BundlerOptionWithStopWorker[] = [];
  const setupErrors: unknown[] = [];
  for (const result of bundlerOptionResults) {
    if (result.status === 'fulfilled') {
      bundlerOptions.push(result.value);
    } else {
      setupErrors.push(result.reason);
    }
  }
  const workerCleanups = collectParallelPluginCleanups(bundlerOptions, setupErrors);
  if (setupErrors.length > 0) {
    return throwWatcherSetupErrorAfterCleanup(
      createSetupError(setupErrors, 'Watcher option setup failed'),
      createWatcherSetupCleanup(workerCleanups),
      'Watcher setup and parallel-plugin worker cleanup failed',
      'Watcher setup and parallel-plugin worker retry cleanup failed',
    );
  }

  warnMultiplePollingOptions(bundlerOptions);
  let runtimeLease: RuntimeLease;
  try {
    runtimeLease = acquireRuntimeLease();
  } catch (error) {
    return throwWatcherSetupErrorAfterCleanup(
      error,
      createWatcherSetupCleanup(workerCleanups),
      'Watcher runtime setup and parallel-plugin worker cleanup failed',
      'Watcher runtime setup and parallel-plugin worker retry cleanup failed',
    );
  }

  let onNativeClose = () => {};
  const callback = createEventCallback(emitter, () => onNativeClose());
  let bindingWatcher: BindingWatcher;
  try {
    bindingWatcher = new BindingWatcher(
      bundlerOptions.map((option) => option.bundlerOptions),
      callback,
    );
  } catch (error) {
    return throwWatcherSetupErrorAfterCleanup(
      error,
      createWatcherSetupCleanup(workerCleanups, runtimeLease),
      'Watcher construction, parallel-plugin worker cleanup, or runtime release failed',
      'Watcher construction and retry cleanup failed',
    );
  }
  const watcher = new Watcher(
    emitter,
    bindingWatcher,
    runtimeLease,
    bundlerOptions.map((option) => option.stopWorkers),
  );
  try {
    onNativeClose = () => watcher.onNativeClose();
    watcher.start();
    emitter.bindClose(() => watcher.close());
  } catch (error) {
    onNativeClose = () => {};
    const cleanupErrors: unknown[] = [];
    let cleanupResult = await watcher.cleanupAfterSetupFailure();
    cleanupErrors.push(...cleanupResult.errors);
    if (cleanupResult.retryable) {
      cleanupResult = await watcher.cleanupAfterSetupFailure();
      for (const cleanupError of cleanupResult.errors) {
        if (!cleanupErrors.includes(cleanupError)) cleanupErrors.push(cleanupError);
      }
    }
    // Keep ownership reachable through the public emitter so a later close
    // can retry any worker termination or runtime release that still failed.
    emitter.bindClose(() => watcher.close());
    if (cleanupErrors.length > 0) {
      throw new AggregateError(
        [error, ...cleanupErrors],
        'Watcher setup, native cleanup, parallel-plugin worker cleanup, or runtime release failed',
      );
    }
    throw error;
  }
}

function collectParallelPluginCleanups(
  bundlerOptions: BundlerOptionWithStopWorker[],
  setupErrors: unknown[],
): RetryableCleanup[] {
  const cleanups = new Set<RetryableCleanup>();
  for (const option of bundlerOptions) {
    if (option.stopWorkers) cleanups.add(option.stopWorkers);
  }
  for (const error of setupErrors) {
    const cleanup = getRetryableCleanup(error);
    if (cleanup) cleanups.add(cleanup);
  }
  return [...cleanups];
}

function createWatcherSetupCleanup(
  initialWorkerCleanups: RetryableCleanup[],
  initialRuntimeLease?: RuntimeLease,
): RetryableCleanup | undefined {
  if (initialWorkerCleanups.length === 0 && !initialRuntimeLease) return undefined;

  let workerCleanups = initialWorkerCleanups;
  let runtimeLease = initialRuntimeLease;
  const cleanup: RetryableCleanup = async () => {
    const errors: unknown[] = [];
    const ownedWorkerCleanups = workerCleanups;
    const workerResults = await Promise.allSettled(
      ownedWorkerCleanups.map((stopWorkers) => runRetryableCleanup(stopWorkers, false)),
    );
    workerCleanups = ownedWorkerCleanups.filter(
      (stopWorkers, index) =>
        workerResults[index].status === 'rejected' && hasRetryableCleanupOwnership(stopWorkers),
    );
    for (const result of workerResults) {
      if (result.status === 'rejected') errors.push(result.reason);
    }

    const ownedRuntimeLease = runtimeLease;
    try {
      ownedRuntimeLease?.release();
      if (runtimeLease === ownedRuntimeLease) {
        runtimeLease = undefined;
      }
    } catch (error) {
      errors.push(error);
    }

    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) {
      throw new AggregateError(
        errors,
        'Watcher parallel-plugin worker cleanup or runtime release failed',
      );
    }
  };
  trackRetryableCleanupOwnership(
    cleanup,
    () => workerCleanups.length > 0 || runtimeLease !== undefined,
  );
  return cleanup;
}

async function throwWatcherSetupErrorAfterCleanup(
  error: unknown,
  cleanup: RetryableCleanup | undefined,
  message: string,
  retryMessage: string,
): Promise<never> {
  if (!cleanup) throw error;
  try {
    await runRetryableCleanup(cleanup);
  } catch (cleanupError) {
    return retryCleanupFromError(
      createCleanupFailureError(error, cleanupError, cleanup, message),
      retryMessage,
    );
  }
  throw error;
}

function createSetupError(errors: unknown[], message: string): unknown {
  return errors.length === 1 ? errors[0] : new AggregateError(errors, message);
}

function warnMultiplePollingOptions(bundlerOptions: BundlerOptionWithStopWorker[]) {
  let found = false;
  for (const option of bundlerOptions) {
    const watch = option.inputOptions.watch;
    const watcher =
      watch && typeof watch === 'object' ? (watch.watcher ?? watch.notify) : undefined;
    if (watcher && (watcher.usePolling != null || watcher.pollInterval != null)) {
      if (found) {
        option.onLog(LOG_LEVEL_WARN, logMultipleWatcherOption());
        return;
      }
      found = true;
    }
  }
}
