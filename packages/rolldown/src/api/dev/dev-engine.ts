import { BindingDevEngine, type BindingHmrUpdate } from '../../binding';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import { createBundlerOptions } from '../../utils/create-bundler-option';
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

    const bindingDevOptions = {
      onHmrUpdates: devOptions.onHmrUpdates,
      watch: devOptions.watch && {
        usePolling: devOptions.watch.usePolling,
        pollInterval: devOptions.watch.pollInterval,
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

  async ensureLatestBuild(): Promise<void> {
    await this.#inner.ensureLatestBuild();
  }

  async invalidate(
    file: string,
    firstInvalidatedBy?: string,
  ): Promise<BindingHmrUpdate> {
    return this.#inner.invalidate(file, firstInvalidatedBy);
  }
}
