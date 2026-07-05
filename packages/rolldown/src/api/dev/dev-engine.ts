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
import { PluginDriver } from '../../plugin/plugin-driver';
import { createBundlerOptions } from '../../utils/create-bundler-option';
import { normalizeBindingResult, unwrapBindingResult } from '../../utils/error';
import { normalizedStringOrRegex } from '../../utils/normalize-string-or-regex';
import { transformToRollupOutput } from '../../utils/transform-to-rollup-output';
import {
  acquireRuntimeLease,
  CloseCoordinator,
  type CloseAttemptResult,
  type RuntimeLease,
} from '../../runtime-lifecycle';
import type { DevOptions } from './dev-options';

export class DevEngine {
  #inner: BindingDevEngine;
  #runtimeLease: RuntimeLease;
  #stopWorkers: (() => Promise<void>) | undefined;
  #nativeClosePromise: Promise<void> | undefined;
  #closeCoordinator = new CloseCoordinator(
    'Dev engine native close, parallel-plugin worker shutdown, or runtime release failed',
  );
  #cachedBuildFinishPromise: Promise<void> | null = null;

  static async create(
    inputOptions: InputOptions,
    outputOptions: OutputOptions = {},
    devOptions: DevOptions = {},
  ): Promise<DevEngine> {
    inputOptions = await PluginDriver.callOptionsHook(inputOptions);
    const options = await createBundlerOptions(inputOptions, outputOptions, false);

    const userOnHmrUpdates = devOptions.onHmrUpdates;
    const bindingOnHmrUpdates: BindingDevOptions['onHmrUpdates'] = userOnHmrUpdates
      ? function (rawResult: BindingResult<[BindingClientHmrUpdate[], string[]]>) {
          const result = normalizeBindingResult(rawResult);
          if (result instanceof Error) {
            userOnHmrUpdates(result);
            return;
          }
          const [updates, changedFiles] = result;
          userOnHmrUpdates({
            updates,
            changedFiles,
          });
        }
      : undefined;

    const userOnOutput = devOptions.onOutput;
    const bindingOnOutput: BindingDevOptions['onOutput'] = userOnOutput
      ? function (rawResult) {
          const result = normalizeBindingResult(rawResult);
          if (result instanceof Error) {
            userOnOutput(result);
            return;
          }
          userOnOutput(transformToRollupOutput(result));
        }
      : undefined;

    const userOnAdditionalAssets = devOptions.onAdditionalAssets;
    const bindingOnAdditionalAssets: BindingDevOptions['onAdditionalAssets'] =
      userOnAdditionalAssets
        ? function (output) {
            userOnAdditionalAssets(transformToRollupOutput(output));
          }
        : undefined;

    const bindingDevOptions: BindingDevOptions = {
      onHmrUpdates: bindingOnHmrUpdates,
      onOutput: bindingOnOutput,
      onAdditionalAssets: bindingOnAdditionalAssets,
      rebuildStrategy: devOptions.rebuildStrategy
        ? devOptions.rebuildStrategy === 'always'
          ? BindingRebuildStrategy.Always
          : devOptions.rebuildStrategy === 'auto'
            ? BindingRebuildStrategy.Auto
            : BindingRebuildStrategy.Never
        : undefined,
      watch: devOptions.watch && {
        skipWrite: devOptions.watch.skipWrite,
        usePolling: devOptions.watch.usePolling,
        pollInterval: devOptions.watch.pollInterval,
        useDebounce: devOptions.watch.useDebounce,
        debounceDuration: devOptions.watch.debounceDuration,
        compareContentsForPolling: devOptions.watch.compareContentsForPolling,
        debounceTickRate: devOptions.watch.debounceTickRate,
        include: normalizedStringOrRegex(devOptions.watch.include),
        exclude: normalizedStringOrRegex(devOptions.watch.exclude),
      },
    };

    let runtimeLease: RuntimeLease;
    try {
      runtimeLease = acquireRuntimeLease();
    } catch (error) {
      try {
        await options.stopWorkers?.();
      } catch (cleanupError) {
        throw new AggregateError(
          [error, cleanupError],
          'Dev engine runtime setup and parallel-plugin worker cleanup both failed',
        );
      }
      throw error;
    }

    try {
      const inner = new BindingDevEngine(options.bundlerOptions, bindingDevOptions);
      return new DevEngine(inner, runtimeLease, options.stopWorkers);
    } catch (error) {
      const errors = [error];
      try {
        await options.stopWorkers?.();
      } catch (cleanupError) {
        errors.push(cleanupError);
      }
      try {
        runtimeLease.release();
      } catch (cleanupError) {
        errors.push(cleanupError);
      }
      throw errors.length === 1
        ? error
        : new AggregateError(errors, 'Dev engine setup and cleanup failed');
    }
  }

  private constructor(
    inner: BindingDevEngine,
    runtimeLease: RuntimeLease,
    stopWorkers: (() => Promise<void>) | undefined,
  ) {
    this.#inner = inner;
    this.#runtimeLease = runtimeLease;
    this.#stopWorkers = stopWorkers;
  }

  async run(): Promise<void> {
    await this.#inner.run();
  }

  async ensureCurrentBuildFinish(): Promise<void> {
    if (this.#cachedBuildFinishPromise) {
      return this.#cachedBuildFinishPromise;
    }
    const promise = this.#inner.ensureCurrentBuildFinish().finally(() => {
      if (this.#cachedBuildFinishPromise === promise) {
        this.#cachedBuildFinishPromise = null;
      }
    });
    this.#cachedBuildFinishPromise = promise;
    return promise;
  }

  async getBundleState(): Promise<BindingBundleState> {
    return this.#inner.getBundleState();
  }

  async ensureLatestBuildOutput(): Promise<void> {
    unwrapBindingResult(await this.#inner.ensureLatestBuildOutput());
  }

  triggerFullBuild(): void {
    this.#inner.triggerFullBuild();
  }

  async invalidate(file: string, firstInvalidatedBy?: string): Promise<BindingClientHmrUpdate[]> {
    return unwrapBindingResult(await this.#inner.invalidate(file, firstInvalidatedBy));
  }

  async registerModules(clientId: string, modules: string[]): Promise<void> {
    await this.#inner.registerModules(clientId, modules);
  }

  async removeClient(clientId: string): Promise<void> {
    await this.#inner.removeClient(clientId);
  }

  close(): Promise<void> {
    return this.#closeCoordinator.close(() => this.#close());
  }

  async #close(): Promise<CloseAttemptResult> {
    const errors: unknown[] = [];
    let retryable = false;
    this.#nativeClosePromise ??= (async () => this.#inner.close())();
    try {
      // Native close waits for any active build and its closeBundle hooks.
      // Parallel-plugin workers must remain alive until that phase settles.
      await this.#nativeClosePromise;
    } catch (error) {
      errors.push(error);
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
    return this.#inner.compileEntry(moduleId, clientId);
  }
}
