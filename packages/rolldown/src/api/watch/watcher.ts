import {
  type BindingWatcherBundler,
  type BindingWatcherEvent,
  BindingWatcher,
} from '../../binding.cjs';
import { LOG_LEVEL_WARN } from '../../log/logging';
import { logMultipleWatcherOption } from '../../log/logs';
import type { CloseCallbackScope } from '../../utils/close-callback-scope';
import { aggregateBindingErrorsIntoJsError, normalizeBindingError } from '../../utils/error';
import type { WatchOptions } from '../../options/watch-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import {
  acquireRuntimeLease,
  CloseCoordinator,
  type CloseAttemptResult,
  type RuntimeLease,
  throwCloseErrors,
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
  waitForRetryableCleanupTurn,
} from '../../utils/retryable-cleanup';
import { arraify } from '../../utils/misc';
import type { WatcherEmitter } from './watch-emitter';

interface WatchResultClose {
  close: () => Promise<void>;
  closeIdentity: string;
}

interface WatcherCloseAttemptResult extends CloseAttemptResult {
  nativeCloseReturned: boolean;
}

interface WatcherCloseAttemptContext {
  automaticNativeCloseRetryScheduled: boolean;
  publiclyObserved: boolean;
  retryNativeCloseAutomatically: boolean;
}

interface RetainedWorkerDiagnostic {
  attempt: WatcherCloseAttemptContext;
  error: unknown;
}

export class WatchResultCloseRegistry {
  #current = new Map<number, WatchResultClose>();
  #pendingBuild = new Map<number, WatchResultClose>();
  #superseded = new Set<WatchResultClose>();
  #terminalOutcomes: Promise<PromiseSettledResult<void>[]> | undefined;

  register(taskIndex: number, closeIdentity: string, close: () => Promise<void>): () => void {
    const resultClose = { close, closeIdentity };
    const currentClose = this.#current.get(taskIndex);
    if (currentClose) {
      this.#superseded.add(currentClose);
    }
    this.#current.set(taskIndex, resultClose);
    let registered = true;
    return () => {
      if (!registered) return;
      registered = false;
      if (this.#current.get(taskIndex) === resultClose) {
        this.#current.delete(taskIndex);
      }
      if (this.#pendingBuild.get(taskIndex) === resultClose) {
        this.#pendingBuild.delete(taskIndex);
      }
      this.#superseded.delete(resultClose);
    };
  }

  beginTaskBuild(taskIndex: number): (buildWillStart: boolean) => void {
    const currentClose = this.#current.get(taskIndex);
    if (!currentClose) return () => {};
    this.#current.delete(taskIndex);
    this.#pendingBuild.set(taskIndex, currentClose);
    let active = true;
    return (buildWillStart) => {
      if (!active) return;
      active = false;
      if (this.#pendingBuild.get(taskIndex) !== currentClose) return;
      this.#pendingBuild.delete(taskIndex);
      if (buildWillStart) {
        this.#superseded.add(currentClose);
      } else {
        this.#current.set(taskIndex, currentClose);
      }
    };
  }

  cancelPendingBuilds(): void {
    for (const [taskIndex, close] of this.#pendingBuild) {
      this.#current.set(taskIndex, close);
    }
    this.#pendingBuild.clear();
  }

  drain(
    nativeOwnedCloseIdentities: ReadonlySet<string> = new Set(),
    includeCurrentAndPending = false,
  ): Promise<PromiseSettledResult<void>[]> {
    if (!this.#terminalOutcomes) {
      const registeredCloses = new Set(this.#superseded);
      if (includeCurrentAndPending) {
        for (const close of this.#current.values()) registeredCloses.add(close);
        for (const close of this.#pendingBuild.values()) registeredCloses.add(close);
      }
      const closes = [...registeredCloses].filter(
        ({ closeIdentity }) => !nativeOwnedCloseIdentities.has(closeIdentity),
      );
      this.clear();
      // Publish the terminal promise before invoking user callbacks so
      // concurrent drain observers cannot start the closures twice.
      this.#terminalOutcomes = Promise.resolve().then(() =>
        Promise.allSettled(closes.map(async ({ close }) => close())),
      );
    }
    return this.#terminalOutcomes;
  }

  clear(): void {
    this.#current.clear();
    this.#pendingBuild.clear();
    this.#superseded.clear();
  }
}

// See internal-docs/watch-mode/implementation.md for the reentrant close cycle.
function wrapWatchResultClose(
  result: BindingWatcherBundler,
  taskIndex: number,
  closeIdentity: string,
  closeCallbackScope: CloseCallbackScope,
  registerClose: (
    taskIndex: number,
    closeIdentity: string,
    close: () => Promise<void>,
  ) => () => void,
): BindingWatcherBundler {
  const close = result.close.bind(result);
  let closePromise: Promise<void> | undefined;
  let unregisterClose = () => {};
  const wrappedClose = () => {
    if (!closePromise) {
      try {
        closePromise = close();
      } catch (error) {
        closePromise = Promise.reject(error);
      }
      void closePromise.then(unregisterClose, () => {});
    }
    return closeCallbackScope.selectClosePromise(closePromise, closeIdentity);
  };
  unregisterClose = registerClose(taskIndex, closeIdentity, wrappedClose);
  Object.defineProperty(result, 'close', {
    configurable: true,
    value: wrappedClose,
    writable: true,
  });
  return result;
}

function createEventCallback(
  emitter: WatcherEmitter,
  onNativeClose: () => void,
  registerResultClose: (
    taskIndex: number,
    closeIdentity: string,
    close: () => Promise<void>,
  ) => () => void,
  beginTaskBuild: (taskIndex: number) => (buildWillStart: boolean) => void,
): (event: BindingWatcherEvent) => Promise<void> {
  return async (event: BindingWatcherEvent) => {
    switch (event.eventKind()) {
      case 'event': {
        const code = event.bundleEventKind();
        if (code === 'BUNDLE_END') {
          const { closeIdentity, duration, output, result, taskIndex } = event.bundleEndData();
          await emitter.emit('event', {
            code: 'BUNDLE_END',
            duration,
            output: [output],
            result: wrapWatchResultClose(
              result,
              taskIndex,
              closeIdentity,
              emitter.closeCallbackScope,
              registerResultClose,
            ),
          });
        } else if (code === 'ERROR') {
          const data = event.bundleErrorData();
          await emitter.emit('event', {
            code: 'ERROR',
            error: aggregateBindingErrorsIntoJsError(data.error),
            result: wrapWatchResultClose(
              data.result,
              data.taskIndex,
              data.closeIdentity,
              emitter.closeCallbackScope,
              registerResultClose,
            ),
          });
        } else if (code === 'BUNDLE_START') {
          const finishTaskBuildStart = beginTaskBuild(event.bundleStartData().taskIndex);
          try {
            await emitter.emit('event', { code: 'BUNDLE_START' });
          } catch (error) {
            finishTaskBuildStart(false);
            throw error;
          }
          finishTaskBuildStart(true);
        } else {
          await emitter.emit('event', { code: code as 'START' | 'END' });
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
  runOutcomePromise: Promise<unknown[]> | undefined;
  settledRunOutcomeErrors: unknown[] | undefined;
  nativeCloseResultPromise:
    | Promise<{
        errors: unknown[];
        nativeCloseReturned: boolean;
        nativeOwnedCloseIdentities: string[];
      }>
    | undefined;
  nativeClosePromise: Promise<void> | undefined;
  closeEventPromise: Promise<void> | undefined;
  resultCloses = new WatchResultCloseRegistry();
  // See internal-docs/watch-mode/implementation.md.
  private closeAttemptContexts = new WeakMap<Promise<void>, WatcherCloseAttemptContext>();
  private automaticNativeCloseRetryAttempted = false;
  private retainedWorkerDiagnostics: RetainedWorkerDiagnostic[] = [];
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
      const runOutcomePromise = Promise.resolve()
        .then(() => this.run())
        .then<unknown[], unknown[]>(
          () => {
            const errors: unknown[] = [];
            this.settledRunOutcomeErrors = errors;
            return errors;
          },
          (error) => {
            const errors = [error];
            this.settledRunOutcomeErrors = errors;
            return errors;
          },
        );
      this.runOutcomePromise = runOutcomePromise;
      void runOutcomePromise.then((errors) => {
        if (errors.length === 0) return;
        // Preserve the failure for a later public close while ensuring the
        // native watcher, workers, and runtime lease are not abandoned.
        this.closeAutomatically();
      });
    }, 0);
  }

  close(): Promise<void> {
    return this.requestClose(true);
  }

  private closeAutomatically(): void {
    const closePromise = this.requestClose(false);
    const context = this.closeAttemptContexts.get(closePromise)!;
    void closePromise
      .catch(async () => {
        if (
          context.publiclyObserved ||
          !context.retryNativeCloseAutomatically ||
          context.automaticNativeCloseRetryScheduled ||
          this.automaticNativeCloseRetryAttempted
        ) {
          return;
        }
        context.automaticNativeCloseRetryScheduled = true;
        this.automaticNativeCloseRetryAttempted = true;
        await waitForRetryableCleanupTurn();
        if (!context.publiclyObserved) {
          this.closeAutomatically();
        }
      })
      .catch(() => {});
  }

  private requestClose(publiclyObserved: boolean): Promise<void> {
    // Native bundle construction starts only after the BUNDLE_START callback
    // returns. A close from that callback keeps the previous result native-owned.
    this.resultCloses.cancelPendingBuilds();
    this.startNativeClose();
    const attemptContext: WatcherCloseAttemptContext = {
      automaticNativeCloseRetryScheduled: false,
      publiclyObserved,
      retryNativeCloseAutomatically: false,
    };
    const closePromise = this.closeCoordinator.close(() => this.closeLifecycle(attemptContext));
    const activeAttemptContext = this.closeAttemptContexts.get(closePromise);
    if (activeAttemptContext) {
      if (publiclyObserved) {
        this.markCloseAttemptPubliclyObserved(activeAttemptContext);
      }
    } else {
      this.closeAttemptContexts.set(closePromise, attemptContext);
    }
    return closePromise;
  }

  private markCloseAttemptPubliclyObserved(context: WatcherCloseAttemptContext): void {
    if (context.publiclyObserved) return;
    context.publiclyObserved = true;
    context.retryNativeCloseAutomatically = false;
    this.retainedWorkerDiagnostics = this.retainedWorkerDiagnostics.filter(
      (diagnostic) => diagnostic.attempt !== context,
    );
  }

  onNativeClose(): void {
    // Native close can be observed without a public caller (for example if
    // the coordinator exits independently). Preserve undelivered worker
    // diagnostics for a later `close()` call while avoiding an unhandled
    // rejection.
    this.closeAutomatically();
  }

  registerResultClose(
    taskIndex: number,
    closeIdentity: string,
    close: () => Promise<void>,
  ): () => void {
    return this.resultCloses.register(taskIndex, closeIdentity, close);
  }

  beginTaskBuild(taskIndex: number): (buildWillStart: boolean) => void {
    return this.resultCloses.beginTaskBuild(taskIndex);
  }

  private async closeLifecycle(context: WatcherCloseAttemptContext): Promise<CloseAttemptResult> {
    const result = await this.closeOwnedResources(context);
    if (!result.nativeCloseReturned) {
      context.retryNativeCloseAutomatically = !context.publiclyObserved;
      return result;
    }

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

    const terminalErrors = this.retainedWorkerDiagnostics.map(({ error }) => error);
    if (terminalErrors.length > 0) {
      result.terminalErrors = terminalErrors;
    }
    return result;
  }

  async cleanupAfterSetupFailure(): Promise<CloseAttemptResult> {
    const result = await this.closeOwnedResources();
    if (!result.nativeCloseReturned) {
      return result;
    }
    try {
      this.runtimeLease.release();
    } catch (error) {
      result.errors.push(error);
      result.retryable = true;
    }
    return result;
  }

  private async closeOwnedResources(
    context?: WatcherCloseAttemptContext,
  ): Promise<WatcherCloseAttemptResult> {
    this.closed = true;
    const errors: unknown[] = [];
    this.cancelScheduledRun(errors);
    this.startNativeClose();
    const nativeCloseResultPromise = this.nativeCloseResultPromise!;
    const nativeCloseResult = await nativeCloseResultPromise;
    if (!nativeCloseResult.nativeCloseReturned) {
      if (this.settledRunOutcomeErrors) {
        errors.push(...this.settledRunOutcomeErrors);
      }
      errors.push(...nativeCloseResult.errors);
      if (this.nativeCloseResultPromise === nativeCloseResultPromise) {
        this.nativeCloseResultPromise = undefined;
        this.nativeClosePromise = undefined;
      }
      return { errors, nativeCloseReturned: false, retryable: true };
    }

    if (this.runOutcomePromise) {
      errors.push(...(this.settledRunOutcomeErrors ?? (await this.runOutcomePromise)));
    }
    errors.push(...(await this.emitter.setupFailureReportErrors()));
    errors.push(...nativeCloseResult.errors);

    // A structured native shutdown owns each task's current bundle handle, so
    // only superseded handles close here.
    const resultCloseOutcomes = await this.resultCloses.drain(
      new Set(nativeCloseResult.nativeOwnedCloseIdentities),
    );
    for (const outcome of resultCloseOutcomes) {
      if (outcome.status === 'rejected') {
        errors.push(outcome.reason);
      }
    }

    errors.push(...this.retainedWorkerDiagnostics.map(({ error }) => error));
    const stopWorkers = this.stopWorkers;
    const workerResults = await Promise.allSettled(stopWorkers.map(async (stop) => stop?.()));
    this.stopWorkers = stopWorkers.filter((_, index) => workerResults[index].status === 'rejected');
    let retryable = false;
    const workerErrors: unknown[] = [];
    for (const result of workerResults) {
      if (result.status === 'rejected') {
        errors.push(result.reason);
        workerErrors.push(result.reason);
        retryable = true;
      }
    }
    if (context && !context.publiclyObserved) {
      this.retainedWorkerDiagnostics.push(
        ...workerErrors.map((error) => ({ attempt: context, error })),
      );
    }

    return { errors, nativeCloseReturned: true, retryable };
  }

  private startNativeClose(): void {
    if (!this.nativeCloseResultPromise) {
      let nativeCloseResultPromise: Promise<{
        errors: unknown[];
        nativeCloseReturned: boolean;
        nativeOwnedCloseIdentities: string[];
      }>;
      try {
        nativeCloseResultPromise = this.inner
          .close()
          .then((result) => ({
            errors: result.errors.map(normalizeBindingError),
            nativeCloseReturned: true,
            nativeOwnedCloseIdentities: result.nativeOwnedCloseIdentities,
          }))
          .catch((error: unknown) => ({
            errors: [error],
            nativeCloseReturned: false,
            nativeOwnedCloseIdentities: [],
          }));
      } catch (error) {
        nativeCloseResultPromise = Promise.resolve({
          errors: [error],
          nativeCloseReturned: false,
          nativeOwnedCloseIdentities: [],
        });
      }
      this.nativeCloseResultPromise = nativeCloseResultPromise;
    }
    if (!this.nativeClosePromise) {
      this.nativeClosePromise = this.nativeCloseResultPromise.then(({ errors }) => {
        throwCloseErrors(errors, 'Watcher native close failed');
      });
      // The public close path consumes the flattened errors. This derived
      // rejection exists only for reentrant close listeners and may settle
      // before listener dispatch begins.
      void this.nativeClosePromise.catch(() => {});
    }
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
    this.startNativeClose();
    await this.emitter.emitClose(this.nativeClosePromise!);
  }

  private async run(): Promise<void> {
    try {
      await this.inner.run();
    } catch (error) {
      void this.emitter
        .failSetup(error)
        .catch((reportError) => console.error('watcher setup error listener failed', reportError));
      throw error;
    }
    // The pending native promise keeps Node.js alive. Await it so an unexpected
    // N-API transport rejection enters the normal fail-closed cleanup path
    // instead of becoming an unhandled rejection.
    await this.inner.waitForClose();
  }
}

export async function createWatcher(
  emitter: WatcherEmitter,
  input: WatchOptions | WatchOptions[],
): Promise<void> {
  const options = arraify(input);
  const closeCallbackScope = emitter.closeCallbackScope;
  // Snapshot config entries and relevant watch/output getters before starting
  // options hooks or parallel workers. A later throwing getter must not
  // abandon setup already running for an earlier watch configuration.
  const enabledOptions = materializePresentValues(options).filter(
    (option) => option.watch !== false,
  );
  if (enabledOptions.length === 0) {
    throw new TypeError('watch() requires at least one configuration with watch enabled');
  }
  const optionsWithOutputs = enabledOptions.map((option) => {
    const outputs = materializePresentValues(arraify(option.output || {}));
    return { option, outputs: outputs.length === 0 ? [{}] : outputs };
  });
  const configSetupResults = await Promise.allSettled(
    optionsWithOutputs.map(async ({ option, outputs }) => {
      const inputOptions = await closeCallbackScope.run(() =>
        PluginDriver.callOptionsHook(option, true),
      );
      return Promise.allSettled(
        outputs.map((output, outputIndex) =>
          createBundlerOptions(inputOptions, output, true, closeCallbackScope, outputIndex === 0),
        ),
      );
    }),
  );
  const bundlerOptions: BundlerOptionWithStopWorker[] = [];
  const bundlerOptionsByConfig: BundlerOptionWithStopWorker[][] = [];
  const setupErrors: unknown[] = [];
  for (const configResult of configSetupResults) {
    if (configResult.status === 'rejected') {
      setupErrors.push(configResult.reason);
      continue;
    }
    const configBundlerOptions: BundlerOptionWithStopWorker[] = [];
    for (const outputResult of configResult.value) {
      if (outputResult.status === 'fulfilled') {
        bundlerOptions.push(outputResult.value);
        configBundlerOptions.push(outputResult.value);
      } else {
        setupErrors.push(outputResult.reason);
      }
    }
    bundlerOptionsByConfig.push(configBundlerOptions);
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

  try {
    warnMultiplePollingOptions(bundlerOptionsByConfig);
  } catch (error) {
    return throwWatcherSetupErrorAfterCleanup(
      error,
      createWatcherSetupCleanup(workerCleanups),
      'Watcher warning and parallel-plugin worker cleanup both failed',
      'Watcher warning and parallel-plugin worker retry cleanup both failed',
    );
  }
  let runtimeLease: RuntimeLease;
  try {
    runtimeLease = await acquireRuntimeLease();
  } catch (error) {
    return throwWatcherSetupErrorAfterCleanup(
      error,
      createWatcherSetupCleanup(workerCleanups),
      'Watcher runtime setup and parallel-plugin worker cleanup failed',
      'Watcher runtime setup and parallel-plugin worker retry cleanup failed',
    );
  }

  let onNativeClose = () => {};
  let registerResultClose =
    (_taskIndex: number, _closeIdentity: string, _close: () => Promise<void>) => () => {};
  let beginTaskBuild = (_taskIndex: number) => (_buildWillStart: boolean) => {};
  const callback = createEventCallback(
    emitter,
    () => onNativeClose(),
    (taskIndex, closeIdentity, close) => registerResultClose(taskIndex, closeIdentity, close),
    (taskIndex) => beginTaskBuild(taskIndex),
  );
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
    registerResultClose = (taskIndex, closeIdentity, close) =>
      watcher.registerResultClose(taskIndex, closeIdentity, close);
    beginTaskBuild = (taskIndex) => watcher.beginTaskBuild(taskIndex);
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

function materializePresentValues<T>(values: T[]): T[] {
  const snapshot: T[] = [];
  const length = values.length;
  for (let index = 0; index < length; index++) {
    if (index in values) snapshot.push(values[index]);
  }
  return snapshot;
}

function warnMultiplePollingOptions(bundlerOptionsByConfig: BundlerOptionWithStopWorker[][]) {
  let found = false;
  for (const bundlerOptions of bundlerOptionsByConfig) {
    const option = bundlerOptions[0];
    if (!option) continue;
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
