import {
  BindingBundler,
  shutdownAsyncRuntime,
  startAsyncRuntime,
} from '../../binding.cjs';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import type { HasProperty, TypeAssert } from '../../types/assert';
import type { RolldownOutput } from '../../types/rolldown-output';
import { RolldownOutputImpl } from '../../types/rolldown-output-impl';
import { createBundlerOptions } from '../../utils/create-bundler-option';
import { unwrapBindingResult } from '../../utils/error';
import { validateOption } from '../../utils/validator';

// @ts-expect-error TS2540: the polyfill of `asyncDispose`.
Symbol.asyncDispose ??= Symbol('Symbol.asyncDispose');

/** @category Programmatic APIs */
export class RolldownBuild {
  #inputOptions: InputOptions;
  #bundler: BindingBundler;
  #stopWorkers?: () => Promise<void>;

  static asyncRuntimeShutdown = false;

  constructor(inputOptions: InputOptions) {
    this.#inputOptions = inputOptions;
    this.#bundler = new BindingBundler();
  }

  get closed(): boolean {
    return this.#bundler.closed;
  }

  async generate(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    return this.#build(false, outputOptions);
  }

  async write(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    return this.#build(true, outputOptions);
  }

  /**
   * Close the build and free resources.
   */
  async close(): Promise<void> {
    await this.#stopWorkers?.();
    await this.#bundler.close();
    shutdownAsyncRuntime();
    RolldownBuild.asyncRuntimeShutdown = true;
    this.#stopWorkers = void 0;
  }

  async [Symbol.asyncDispose](): Promise<void> {
    await this.close();
  }

  // TODO(shulaoda)
  // The `watchFiles` method returns a promise, but Rollup does not.
  // Converting it to a synchronous API might cause a deadlock if the user calls `write` and `watchFiles` simultaneously.
  get watchFiles(): Promise<string[]> {
    return Promise.resolve(this.#bundler.getWatchFiles());
  }

  async #build(
    isWrite: boolean,
    outputOptions: OutputOptions,
  ): Promise<RolldownOutput> {
    validateOption('output', outputOptions);
    await this.#stopWorkers?.();
    const option = await createBundlerOptions(
      this.#inputOptions,
      outputOptions,
      false,
    );

    if (RolldownBuild.asyncRuntimeShutdown) {
      startAsyncRuntime();
    }

    try {
      this.#stopWorkers = option.stopWorkers;
      let output: Awaited<ReturnType<BindingBundler['generate']>>;
      if (isWrite) {
        output = await this.#bundler.write(option.bundlerOptions);
      } else {
        output = await this.#bundler.generate(option.bundlerOptions);
      }
      return new RolldownOutputImpl(unwrapBindingResult(output));
    } catch (e) {
      await option.stopWorkers?.();
      throw e;
    }
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>;
}
