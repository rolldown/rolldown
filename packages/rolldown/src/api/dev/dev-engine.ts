import {
  type BindingBundleState,
  type BindingClientHmrUpdate,
  BindingDevEngine,
  type BindingDevOptions,
  BindingRebuildStrategy,
  type BindingResult,
} from '../../binding.cjs';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import { assertParallelPluginOptionsSupported } from '../../plugin/parallel-plugin';
import { PluginDriver } from '../../plugin/plugin-driver';
import {
  createRequiredAsyncContext,
  trackAsyncCallbackSettlement,
} from '../../utils/async-context';
import { createBundlerOptions } from '../../utils/create-bundler-option';
import { CloseCallbackScope, createCloseIdentity } from '../../utils/close-callback-scope';
import {
  normalizeBindingResult,
  normalizeBindingResultErrors,
  unwrapBindingResult,
} from '../../utils/error';
import {
  createCleanupFailureError,
  hasRetryableCleanupOwnership,
  retryCleanupFromError,
  runRetryableCleanup,
  trackRetryableCleanupOwnership,
  type RetryableCleanup,
} from '../../utils/retryable-cleanup';
import { normalizedStringOrRegex } from '../../utils/normalize-string-or-regex';
import { transformToRollupOutput } from '../../utils/transform-to-rollup-output';
import {
  acquireRuntimeLease,
  CloseCoordinator,
  type CloseAttemptResult,
  type RuntimeLease,
  throwCloseErrors,
} from '../../runtime-lifecycle';
import { assertRuntimeFeature } from '../../runtime-support';
import type { DevOptions } from './dev-options';

type BindingDevEngineWithTerminalClose = BindingDevEngine & {
  closeTerminal(): Promise<BindingResult<void>>;
};

interface DevCallbackOwner {}

interface DevCallbackInvocation {
  active: boolean;
  owner: DevCallbackOwner;
  parent: DevCallbackInvocation | undefined;
}

// See internal-docs/async-context/implementation.md.
const devCallbackContext = createRequiredAsyncContext<DevCallbackInvocation>();

export class DevEngine {
  #inner: BindingDevEngineWithTerminalClose;
  #runtimeLease: RuntimeLease;
  #callbackOwner: DevCallbackOwner;
  #stopWorkers: (() => Promise<void>) | undefined;
  #nativeCloseErrorsPromise: Promise<unknown[]> | undefined;
  #closeCallbackScope: CloseCallbackScope;
  #closeIdentity: string;
  #closeCoordinator = new CloseCoordinator(
    'Dev engine native close, parallel-plugin worker shutdown, or runtime release failed',
  );
  #cachedBuildFinishPromise: Promise<void> | null = null;
  // See internal-docs/dev-engine/implementation.md sections 15-16.
  #isClosing = false;
  #activeOperations = 0;
  #operationsDrainedPromise: Promise<void> | undefined;
  #resolveOperationsDrained: (() => void) | undefined;

  static async create(
    inputOptions: InputOptions,
    outputOptions: OutputOptions = {},
    devOptions: DevOptions = {},
  ): Promise<DevEngine> {
    assertRuntimeFeature('dev');
    assertParallelPluginOptionsSupported(inputOptions.plugins, outputOptions.plugins);
    inputOptions = await PluginDriver.callOptionsHook(inputOptions);
    const closeIdentity = createCloseIdentity('dev-engine');
    const closeCallbackScope = new CloseCallbackScope(closeIdentity);
    const options = await createBundlerOptions(
      inputOptions,
      outputOptions,
      false,
      closeCallbackScope,
    );
    const callbackOwner = {};

    let bindingDevOptions: BindingDevOptions;
    try {
      bindingDevOptions = closeCallbackScope.wrapCallbacks(
        createBindingDevOptions(devOptions, callbackOwner),
      );
    } catch (error) {
      return throwDevSetupErrorAfterCleanup(
        error,
        createDevSetupCleanup(options.stopWorkers),
        'Dev engine option setup and parallel-plugin worker cleanup both failed',
        'Dev engine option setup and parallel-plugin worker retry cleanup both failed',
      );
    }

    let runtimeLease: RuntimeLease;
    try {
      runtimeLease = await acquireRuntimeLease();
    } catch (error) {
      return throwDevSetupErrorAfterCleanup(
        error,
        createDevSetupCleanup(options.stopWorkers),
        'Dev engine runtime setup and parallel-plugin worker cleanup both failed',
        'Dev engine runtime setup and parallel-plugin worker retry cleanup both failed',
      );
    }

    try {
      const inner = new BindingDevEngine(
        options.bundlerOptions,
        bindingDevOptions,
      ) as BindingDevEngineWithTerminalClose;
      return new DevEngine(
        inner,
        runtimeLease,
        callbackOwner,
        options.stopWorkers,
        closeCallbackScope,
        closeIdentity,
      );
    } catch (error) {
      return throwDevSetupErrorAfterCleanup(
        error,
        createDevSetupCleanup(options.stopWorkers, runtimeLease),
        'Dev engine setup and cleanup failed',
        'Dev engine setup and retry cleanup failed',
      );
    }
  }

  private constructor(
    inner: BindingDevEngineWithTerminalClose,
    runtimeLease: RuntimeLease,
    callbackOwner: DevCallbackOwner,
    stopWorkers: (() => Promise<void>) | undefined,
    closeCallbackScope: CloseCallbackScope,
    closeIdentity: string,
  ) {
    this.#inner = inner;
    this.#runtimeLease = runtimeLease;
    this.#callbackOwner = callbackOwner;
    this.#stopWorkers = stopWorkers;
    this.#closeCallbackScope = closeCallbackScope;
    this.#closeIdentity = closeIdentity;
  }

  async run(): Promise<void> {
    unwrapDevEngineBindingResult(
      await this.#runOperation(() => this.#inner.run()),
      'Failed to run dev engine',
    );
  }

  async ensureCurrentBuildFinish(): Promise<void> {
    if (this.#isClosing) {
      return;
    }
    if (this.#cachedBuildFinishPromise) {
      return this.#cachedBuildFinishPromise;
    }
    const promise = this.#runOperation(() => this.#inner.ensureCurrentBuildFinish())
      .then((result) =>
        unwrapDevEngineBindingResult(result, 'Failed to ensure current build finish'),
      )
      .finally(() => {
        if (this.#cachedBuildFinishPromise === promise) {
          this.#cachedBuildFinishPromise = null;
        }
      });
    this.#cachedBuildFinishPromise = promise;
    return promise;
  }

  async getBundleState(): Promise<BindingBundleState> {
    return this.#runOperation(() => this.#inner.getBundleState());
  }

  async ensureLatestBuildOutput(): Promise<void> {
    unwrapBindingResult(await this.#runOperation(() => this.#inner.ensureLatestBuildOutput()));
  }

  triggerFullBuild(): void {
    this.#assertOpen();
    this.#inner.triggerFullBuild();
  }

  async invalidate(file: string, firstInvalidatedBy?: string): Promise<BindingClientHmrUpdate[]> {
    return unwrapBindingResult(
      await this.#runOperation(() => this.#inner.invalidate(file, firstInvalidatedBy)),
    );
  }

  async registerModules(clientId: string, modules: string[]): Promise<void> {
    await this.#runOperation(() => this.#inner.registerModules(clientId, modules));
  }

  async removeClient(clientId: string): Promise<void> {
    if (this.#isClosing) {
      return;
    }
    await this.#runOperation(() => this.#inner.removeClient(clientId));
  }

  close(): Promise<void> {
    if (!this.#isClosing && this.#isActiveDevCallback()) {
      return Promise.reject(
        new Error('Cannot close a dev engine from one of its active JavaScript callbacks'),
      );
    }
    if (!this.#isClosing) {
      this.#isClosing = true;
      if (this.#activeOperations > 0) {
        this.#operationsDrainedPromise = new Promise((resolve) => {
          this.#resolveOperationsDrained = resolve;
        });
      }
    }
    return this.#closeCallbackScope.selectClosePromise(
      this.#closeCoordinator.close(() => this.#close()),
      this.#closeIdentity,
    );
  }

  async #close(): Promise<CloseAttemptResult> {
    const errors: unknown[] = [];
    let retryable = false;
    await this.#operationsDrainedPromise;
    const nativeCloseErrorsPromise = (this.#nativeCloseErrorsPromise ??= (async () =>
      normalizeBindingResultErrors(await this.#inner.closeTerminal()))());
    // Native close waits for closeBundle and coordinator shutdown. Keep
    // parallel-plugin workers alive until every native diagnostic is captured.
    try {
      errors.push(...(await nativeCloseErrorsPromise));
    } catch (error) {
      if (this.#nativeCloseErrorsPromise === nativeCloseErrorsPromise) {
        this.#nativeCloseErrorsPromise = undefined;
      }
      return { errors: [error], retryable: true };
    }

    const stopWorkers = this.#stopWorkers;
    try {
      await stopWorkers?.();
      if (this.#stopWorkers === stopWorkers) {
        this.#stopWorkers = undefined;
      }
    } catch (error) {
      errors.push(error);
      retryable = true;
    }

    try {
      this.#runtimeLease.release();
    } catch (error) {
      errors.push(error);
      retryable = true;
    }

    return { errors, retryable };
  }

  /**
   * Compile a lazy entry module and return HMR-style patch code.
   *
   * This is called when a dynamically imported module is first requested at runtime.
   * The module was previously stubbed with a proxy, and now we need to compile the
   * actual module and its dependencies.
   *
   * @param moduleId - The absolute file path of the module to compile
   * @param clientId - The client ID requesting this compilation
   * @returns The compiled JavaScript code as a string (HMR patch format)
   */
  async compileEntry(moduleId: string, clientId: string): Promise<string> {
    return unwrapDevEngineBindingResult(
      await this.#runOperation(() => this.#inner.compileEntry(moduleId, clientId)),
      'Failed to compile lazy entry',
    );
  }

  #assertOpen(): void {
    if (this.#isClosing) {
      throw new Error('Dev engine is closed');
    }
  }

  #isActiveDevCallback(): boolean {
    let invocation = devCallbackContext.getStore();
    while (invocation) {
      if (invocation.owner === this.#callbackOwner && invocation.active) return true;
      invocation = invocation.parent;
    }
    return false;
  }

  async #runOperation<T>(operation: () => Promise<T>): Promise<T> {
    this.#assertOpen();
    this.#activeOperations += 1;
    try {
      return await operation();
    } finally {
      this.#activeOperations -= 1;
      if (this.#isClosing && this.#activeOperations === 0) {
        this.#resolveOperationsDrained?.();
        this.#resolveOperationsDrained = undefined;
      }
    }
  }
}

function unwrapDevEngineBindingResult<T>(result: BindingResult<T>, aggregateMessage: string): T {
  throwCloseErrors(normalizeBindingResultErrors(result), aggregateMessage);
  return result as T;
}

function createBindingDevOptions(
  devOptions: DevOptions,
  callbackOwner: DevCallbackOwner,
): BindingDevOptions {
  const userOnHmrUpdates = devOptions.onHmrUpdates;
  const bindingOnHmrUpdates: BindingDevOptions['onHmrUpdates'] = userOnHmrUpdates
    ? function (rawResult: BindingResult<[BindingClientHmrUpdate[], string[]]>) {
        const result = normalizeBindingResult(rawResult);
        if (result instanceof Error) {
          return runDevCallback(callbackOwner, () => userOnHmrUpdates(result));
        }
        const [updates, changedFiles] = result;
        return runDevCallback(callbackOwner, () =>
          userOnHmrUpdates({
            updates,
            changedFiles,
          }),
        );
      }
    : undefined;

  const userOnOutput = devOptions.onOutput;
  const bindingOnOutput: BindingDevOptions['onOutput'] = userOnOutput
    ? function (rawResult) {
        const result = normalizeBindingResult(rawResult);
        if (result instanceof Error) {
          return runDevCallback(callbackOwner, () => userOnOutput(result));
        }
        return runDevCallback(callbackOwner, () => userOnOutput(transformToRollupOutput(result)));
      }
    : undefined;

  const userOnAdditionalAssets = devOptions.onAdditionalAssets;
  const bindingOnAdditionalAssets: BindingDevOptions['onAdditionalAssets'] = userOnAdditionalAssets
    ? function (output) {
        return runDevCallback(callbackOwner, () =>
          userOnAdditionalAssets(transformToRollupOutput(output)),
        );
      }
    : undefined;
  const rebuildStrategy = devOptions.rebuildStrategy;
  const watch = devOptions.watch;

  return {
    onHmrUpdates: bindingOnHmrUpdates,
    onOutput: bindingOnOutput,
    onAdditionalAssets: bindingOnAdditionalAssets,
    rebuildStrategy: bindingifyRebuildStrategy(rebuildStrategy),
    watch: watch && {
      skipWrite: watch.skipWrite,
      usePolling: watch.usePolling,
      pollInterval: watch.pollInterval,
      useDebounce: watch.useDebounce,
      debounceDuration: watch.debounceDuration,
      compareContentsForPolling: watch.compareContentsForPolling,
      debounceTickRate: watch.debounceTickRate,
      include: normalizedStringOrRegex(watch.include),
      exclude: normalizedStringOrRegex(watch.exclude),
    },
  };
}

function bindingifyRebuildStrategy(
  strategy: DevOptions['rebuildStrategy'],
): BindingRebuildStrategy | undefined {
  switch (strategy) {
    case undefined:
      return undefined;
    case 'always':
      return BindingRebuildStrategy.Always;
    case 'auto':
      return BindingRebuildStrategy.Auto;
    case 'never':
      return BindingRebuildStrategy.Never;
    default:
      throw new TypeError(
        `Invalid dev rebuildStrategy ${formatInvalidRebuildStrategy(strategy)}. Expected "always", "auto", or "never".`,
      );
  }
}

function formatInvalidRebuildStrategy(strategy: unknown): string {
  if (strategy === null) return 'null';
  switch (typeof strategy) {
    case 'string':
      return JSON.stringify(strategy);
    case 'bigint':
      return `${strategy}n`;
    case 'boolean':
    case 'number':
    case 'undefined':
      return String(strategy);
    case 'symbol':
      return strategy.toString();
    case 'function':
      return '<function>';
    case 'object':
      return '<object>';
  }
  return '<unknown>';
}

function runDevCallback<T>(owner: DevCallbackOwner, callback: () => T): T {
  const invocation: DevCallbackInvocation = {
    active: true,
    owner,
    parent: devCallbackContext.getStore(),
  };
  const deactivate = () => {
    invocation.active = false;
  };

  return devCallbackContext.run(invocation, () => {
    try {
      return trackAsyncCallbackSettlement(callback(), deactivate);
    } catch (error) {
      deactivate();
      throw error;
    }
  });
}

function createDevSetupCleanup(
  initialStopWorkers: RetryableCleanup | undefined,
  initialRuntimeLease?: RuntimeLease,
): RetryableCleanup | undefined {
  if (!initialStopWorkers && !initialRuntimeLease) return undefined;

  let stopWorkers = initialStopWorkers;
  let runtimeLease = initialRuntimeLease;
  const cleanup: RetryableCleanup = async () => {
    const errors: unknown[] = [];
    const ownedStopWorkers = stopWorkers;
    try {
      if (ownedStopWorkers) {
        await runRetryableCleanup(ownedStopWorkers, false);
      }
      if (stopWorkers === ownedStopWorkers) {
        stopWorkers = undefined;
      }
    } catch (error) {
      if (ownedStopWorkers && !hasRetryableCleanupOwnership(ownedStopWorkers)) {
        stopWorkers = undefined;
      }
      errors.push(error);
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
        'Dev engine parallel-plugin worker cleanup or runtime release failed',
      );
    }
  };
  trackRetryableCleanupOwnership(
    cleanup,
    () => stopWorkers !== undefined || runtimeLease !== undefined,
  );
  return cleanup;
}

async function throwDevSetupErrorAfterCleanup(
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
