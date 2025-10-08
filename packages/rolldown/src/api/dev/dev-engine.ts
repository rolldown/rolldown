import {
  type BindingClientHmrUpdate,
  BindingDevEngine,
  type BindingDevOptions,
  BindingRebuildStrategy,
  type BindingResult,
} from '../../binding';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import { createBundlerOptions } from '../../utils/create-bundler-option';
import { normalizeBindingResult } from '../../utils/error';
import type { DevOptions } from './dev-options';

export class DevEngine {
  #inner: BindingDevEngine;
  #cachedBuildFinishPromise: Promise<void> | null = null;

  static async create(
    inputOptions: InputOptions,
    outputOptions: OutputOptions = {},
    devOptions: DevOptions = {},
  ): Promise<DevEngine> {
    inputOptions = await PluginDriver.callOptionsHook(inputOptions);
    const options = await createBundlerOptions(
      inputOptions,
      outputOptions,
      false,
    );

    const userOnHmrUpdates = devOptions.onHmrUpdates;
    const bindingOnHmrUpdates: BindingDevOptions['onHmrUpdates'] =
      userOnHmrUpdates
        ? function(
          rawResult: BindingResult<[BindingClientHmrUpdate[], string[]]>,
        ) {
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
      ? function(rawResult) {
        userOnOutput(normalizeBindingResult(rawResult));
      }
      : undefined;

    const bindingDevOptions: BindingDevOptions = {
      onHmrUpdates: bindingOnHmrUpdates,
      onOutput: bindingOnOutput,
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
      },
    };

    const inner = new BindingDevEngine(
      options.bundlerOptions,
      bindingDevOptions,
    );

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
    const promise = this.#inner.ensureCurrentBuildFinish()
      .then(() => {
        this.#cachedBuildFinishPromise = null;
      });
    this.#cachedBuildFinishPromise = promise;
    return promise;
  }

  async hasLatestBuildOutput(): Promise<boolean> {
    return this.#inner.hasLatestBuildOutput();
  }

  async ensureLatestBuildOutput(): Promise<void> {
    await this.#inner.ensureLatestBuildOutput();
  }

  async invalidate(
    file: string,
    firstInvalidatedBy?: string,
  ): Promise<BindingClientHmrUpdate[]> {
    return this.#inner.invalidate(file, firstInvalidatedBy);
  }

  registerModules(clientId: string, modules: string[]): void {
    this.#inner.registerModules(clientId, modules);
  }

  removeClient(clientId: string): void {
    this.#inner.removeClient(clientId);
  }

  async close(): Promise<void> {
    await this.#inner.close();
  }
}
