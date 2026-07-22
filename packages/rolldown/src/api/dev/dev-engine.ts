import {
  type BindingBundleState,
  type BindingClientHmrUpdate,
  BindingDevEngine,
  type BindingDevOptions,
  type BindingLazyChunkOutput,
  BindingRebuildStrategy,
  type BindingResult,
  shutdownAsyncRuntime,
  startAsyncRuntime,
} from '../../binding.cjs';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import { createBundlerOptions } from '../../utils/create-bundler-option';
import { normalizeBindingResult, unwrapBindingResult } from '../../utils/error';
import { normalizedStringOrRegex } from '../../utils/normalize-string-or-regex';
import { transformToRollupOutput } from '../../utils/transform-to-rollup-output';
import type { DevOptions } from './dev-options';

export class DevEngine {
  #inner: BindingDevEngine;
  #cachedBuildFinishPromise: Promise<void> | null = null;
  #asyncRuntimeReleased = false;

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

    const inner = new BindingDevEngine(options.bundlerOptions, bindingDevOptions);

    startAsyncRuntime();

    return new DevEngine(inner);
  }

  private constructor(inner: BindingDevEngine) {
    this.#inner = inner;
  }

  async run(): Promise<void> {
    await this.#inner.run();
  }

  async ensureCurrentBuildFinish(): Promise<void> {
    if (this.#cachedBuildFinishPromise) {
      return this.#cachedBuildFinishPromise;
    }
    const promise = this.#inner.ensureCurrentBuildFinish().then(() => {
      this.#cachedBuildFinishPromise = null;
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

  /**
   * Client-connect signal (the clientId hello): creates the per-client session
   * with an empty ship map. Reconnects arrive as fresh clientIds.
   */
  async registerClient(clientId: string): Promise<void> {
    await this.#inner.registerClient(clientId);
  }

  /**
   * Delivery notification from the serving middleware: the response for
   * `filename` completed, so record its modules as shipped to that client.
   */
  async notifyPayloadDelivered(filename: string): Promise<void> {
    await this.#inner.notifyPayloadDelivered(filename);
  }

  async removeClient(clientId: string): Promise<void> {
    await this.#inner.removeClient(clientId);
  }

  async close(): Promise<void> {
    // Claim the release before the first await so a second `close` cannot release twice.
    const shouldRelease = !this.#asyncRuntimeReleased;
    this.#asyncRuntimeReleased = true;
    try {
      await this.#inner.close();
    } finally {
      if (shouldRelease) {
        shutdownAsyncRuntime();
      }
    }
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
   * @returns The compiled chunk: its code plus the filename whose delivery the
   * serving middleware reports via {@link notifyPayloadDelivered}
   */
  async compileEntry(moduleId: string, clientId: string): Promise<BindingLazyChunkOutput> {
    return this.#inner.compileEntry(moduleId, clientId);
  }
}
