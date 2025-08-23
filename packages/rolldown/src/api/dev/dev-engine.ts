import { BindingDevEngine } from '../../binding';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import { createBundlerOptions } from '../../utils/create-bundler-option';

export class DevEngine {
  #inner: BindingDevEngine;
  #cachedBuildFinishPromise: Promise<void> | null = null;

  static async create(
    inputOptions: InputOptions,
    outputOptions: OutputOptions,
  ): Promise<DevEngine> {
    inputOptions = await PluginDriver.callOptionsHook(inputOptions);
    const options = await createBundlerOptions(
      inputOptions,
      outputOptions,
      false,
    );

    const inner = new BindingDevEngine(options.bundlerOptions);

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
}
