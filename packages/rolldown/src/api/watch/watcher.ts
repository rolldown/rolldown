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
  CloseCoordinator,
  type CloseAttemptResult,
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
import { findPropertyDescriptorInPrototypeChain } from '../../utils/prototype-chain';
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
  ): Promise<PromiseSettledResult<void>[]> {
    if (!this.#terminalOutcomes) {
      const registeredCloses = new Set(this.#superseded);
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
        // The native coordinator awaits this callback, so dispatching
        // listeners here would make a close listener calling `watcher.close()`
        // self-await the coordinator. Start the JS close lifecycle without
        // awaiting listener dispatch.
        onNativeClose();
        break;
    }
  };
}

class Watcher {
  inner: BindingWatcher;
  emitter: WatcherEmitter;
  stopWorkers: ((() => Promise<void>) | undefined)[];
  releaseOptionBoxes: (() => void)[];
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
    'Watcher native close, parallel-plugin worker shutdown, or close listener failed',
  );

  constructor(
    emitter: WatcherEmitter,
    inner: BindingWatcher,
    stopWorkers: ((() => Promise<void>) | undefined)[],
    releaseOptionBoxes: (() => void)[],
  ) {
    this.inner = inner;
    this.emitter = emitter;
    this.stopWorkers = stopWorkers;
    this.releaseOptionBoxes = releaseOptionBoxes;
  }

  start(): void {
    // Defer so watch() returns the emitter before the first build,
    // giving the caller a chance to attach .on() handlers.
    // A timer is a host turn in both browsers and Node.js.
    this.scheduledRun = globalThis.setTimeout(() => {
      this.scheduledRun = undefined;
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
        // native watcher and its workers are not abandoned.
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
    // Native close can be observed without a public caller (the coordinator
    // may exit on its own), so keep undelivered worker diagnostics for a later
    // `close()` without producing an unhandled rejection.
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

    const terminalErrors = this.retainedWorkerDiagnostics.map(({ error }) => error);
    if (terminalErrors.length > 0) {
      result.terminalErrors = terminalErrors;
    }
    return result;
  }

  private async closeOwnedResources(
    context?: WatcherCloseAttemptContext,
  ): Promise<WatcherCloseAttemptResult> {
    const errors: unknown[] = [];
    this.cancelScheduledRun();
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
    // Rebuild-cycle invalidates only ran after successful rebuilds, so boxes
    // stranded by failed rebuilds (and by hooks after the last invalidate) are
    // released here. Idempotent, no-op outside the threadless-WASI flavor.
    for (const release of this.releaseOptionBoxes) {
      try {
        release();
      } catch (error) {
        errors.push(error);
      }
    }
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

  private cancelScheduledRun(): void {
    if (this.scheduledRun === undefined) return;
    const scheduledRun = this.scheduledRun;
    this.scheduledRun = undefined;
    globalThis.clearTimeout(scheduledRun);
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
  // One read of `watch` per configuration and per later shape of it: the
  // enablement filter here, `bindingifyInputOptions` and
  // `warnMultiplePollingOptions` each used to re-run an accessor-backed
  // `watch`, so a getter that answers once had its `skipWrite` and `watcher`
  // settings silently dropped - the very settings main honours. Everything
  // downstream reads `watch` through a view of the configuration that answers
  // from the live own descriptor and memoises what it had to call.
  const enabledOptions: WatchOptions[] = [];
  for (const option of materializePresentValues(options)) {
    // Walk first and discard the result: the walk is what bounds a cyclic or
    // fabricated prototype chain, and it has to throw here, before the read.
    findPropertyDescriptorInPrototypeChain(option, 'watch', 'inspecting watch options');
    const watch = Reflect.get(option, 'watch', option) as WatchOptions['watch'];
    if (watch === false) continue;
    enabledOptions.push(createWatchOptionSnapshotView(option, watch));
  }
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
      // One entry per input config: how many of the flat per-output options
      // above belong to it. Native shares one filesystem watcher per config
      // group so a save rebuilds every affected output in one START..END
      // envelope (#10613). Setup failures throw before this point, so every
      // group holds at least one output and the sizes sum to the flat length.
      bundlerOptionsByConfig.map((configOptions) => configOptions.length),
    );
  } catch (error) {
    return throwWatcherSetupErrorAfterCleanup(
      error,
      createWatcherSetupCleanup(workerCleanups),
      'Watcher construction or parallel-plugin worker cleanup failed',
      'Watcher construction and retry cleanup failed',
    );
  }
  const watcher = new Watcher(
    emitter,
    bindingWatcher,
    bundlerOptions.map((option) => option.stopWorkers),
    bundlerOptions.map((option) => option.releaseOptionBoxes),
  );
  onNativeClose = () => watcher.onNativeClose();
  registerResultClose = (taskIndex, closeIdentity, close) =>
    watcher.registerResultClose(taskIndex, closeIdentity, close);
  beginTaskBuild = (taskIndex) => watcher.beginTaskBuild(taskIndex);
  watcher.start();
  emitter.bindClose(() => watcher.close());
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
): RetryableCleanup | undefined {
  if (initialWorkerCleanups.length === 0) return undefined;

  let workerCleanups = initialWorkerCleanups;
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

    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) {
      throw new AggregateError(errors, 'Watcher parallel-plugin worker cleanup failed');
    }
  };
  trackRetryableCleanupOwnership(cleanup, () => workerCleanups.length > 0);
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

/**
 * View of one watch configuration whose only difference from the original is
 * that `[[Get]]` of `watch` answers without re-running a getter the enablement
 * filter already ran, so `bindingifyInputOptions` and
 * `warnMultiplePollingOptions` see what the filter saw.
 *
 * The answer is derived from the target's CURRENT own descriptor, never from a
 * slot the traps maintain, so nothing has to be intercepted to keep it honest:
 *
 * - an own data property answers with its value. That is live, so a `watch` an
 *   `options` hook assigns - through the view or through the caller's own
 *   alias - is honoured, and it is exactly what the `get` invariant demands of
 *   a non-configurable non-writable property, so freezing the configuration
 *   afterwards cannot make the next read throw.
 * - an own accessor with no getter answers `undefined`: there is nothing to
 *   call, and `undefined` is what a plain read produces. That is also what the
 *   `get` invariant demands when such a descriptor is non-configurable.
 * - an own accessor with a getter answers from the memo when the memo holds
 *   that same getter's result, and otherwise calls it once and memoises it. A
 *   hook installing a fresh `watch` getter - what
 *   `Object.defineProperty(options, 'watch', { get() { ... } })` does - is
 *   therefore honoured, at one call.
 * - no own descriptor at all - inherited, or deleted - answers from the memo
 *   the filter's read seeded, so deleting an inherited `watch` through the view
 *   costs no second getter call.
 *
 * A getter therefore runs at most once per distinct shape, and every memoised
 * read passes the original as the receiver - the same `this` the filter's read
 * used - so a getter backed by a private field works and cannot re-enter this
 * view. Every other key is delegated with the original as the receiver for the
 * same reason, unless the receiver is some object derived from the view
 * (`Object.create(view)`), which keeps its own.
 *
 * The original configuration IS the `Proxy` target, so `ownKeys`, descriptors,
 * `has`, the prototype, `preventExtensions`, `defineProperty` and `delete` are
 * delegated by construction, and `Object.freeze`, `Object.seal` or
 * `Object.preventExtensions` applied to the view by an `options` hook satisfies
 * the `Proxy` invariants instead of making a later `ownKeys` throw.
 * Redefinition, deletion and setter calls all change the own descriptor, which
 * is the one thing the rule above reads.
 *
 * A plain `Object.create` shadow would hide the configuration's own keys, so an
 * `options` hook doing `{ ...options }` - what Vite's config plugins do - would
 * see nothing but `watch`; rebuilding a plain object from the original's own
 * descriptors would instead drop the fields only a `get` trap can answer.
 *
 * See internal-docs/watch-mode/implementation.md.
 */
function createWatchOptionSnapshotView(
  option: WatchOptions,
  watch: WatchOptions['watch'],
): WatchOptions {
  /** What one read produced, kept under the shape that produced it. */
  type WatchMemo =
    | { kind: 'accessor'; getter: () => unknown; value: WatchOptions['watch'] }
    | { kind: 'none'; value: WatchOptions['watch'] };

  const ownDescriptor = Reflect.getOwnPropertyDescriptor(option, 'watch');
  // oxlint-disable-next-line typescript/unbound-method -- memo key only, never invoked through it
  const ownGetter = ownDescriptor && !('value' in ownDescriptor) ? ownDescriptor.get : undefined;
  // Seed the memo with the read the enablement filter already took. A shape
  // that answers on its own - an own data property, or an own accessor with no
  // getter - never consults the memo, so it seeds none.
  let memo: WatchMemo | undefined = ownGetter
    ? { kind: 'accessor', getter: ownGetter, value: watch }
    : ownDescriptor
      ? undefined
      : { kind: 'none', value: watch };

  // The original is the receiver, the same `this` the filter's read used, so a
  // getter reading a private field works and cannot re-enter this view.
  const readWatch = (): WatchOptions['watch'] =>
    Reflect.get(option, 'watch', option) as WatchOptions['watch'];

  const view = new Proxy(option, {
    get(target, key, receiver) {
      if (key !== 'watch') {
        return Reflect.get(target, key, receiver === view ? target : receiver);
      }
      const descriptor = Reflect.getOwnPropertyDescriptor(target, 'watch');
      if (descriptor && 'value' in descriptor) return descriptor.value;
      // oxlint-disable-next-line typescript/unbound-method -- memo key only, never invoked through it
      const getter = descriptor?.get;
      if (getter) {
        if (!memo || memo.kind !== 'accessor' || memo.getter !== getter) {
          memo = { kind: 'accessor', getter, value: readWatch() };
        }
        return memo.value;
      }
      if (descriptor) return undefined;
      if (!memo || memo.kind !== 'none') memo = { kind: 'none', value: readWatch() };
      return memo.value;
    },
    set(target, key, value, receiver) {
      return Reflect.set(target, key, value, receiver === view ? target : receiver);
    },
  }) as WatchOptions;
  return view;
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
    const watcher = watch && typeof watch === 'object' ? watch.watcher : undefined;
    if (watcher && (watcher.usePolling != null || watcher.pollInterval != null)) {
      if (found) {
        option.onLog(LOG_LEVEL_WARN, logMultipleWatcherOption());
        return;
      }
      found = true;
    }
  }
}
