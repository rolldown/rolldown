import type { BindingBundlerImpl } from '../../binding';
import {
  BindingBundler,
  shutdownAsyncRuntime,
  startAsyncRuntime,
} from '../../binding';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import type { HasProperty, TypeAssert } from '../../types/assert';
import type { RolldownOutput } from '../../types/rolldown-output';
import { createBundlerOptions } from '../../utils/create-bundler-option';
import {
  handleOutputErrors,
  transformToRollupOutput,
} from '../../utils/transform-to-rollup-output';
import { validateOption } from '../../utils/validator';

interface BundlerImplWithStopWorker {
  impl: BindingBundlerImpl;
  stopWorkers?: () => Promise<void>;
  shutdown: () => void;
}

// @ts-expect-error TS2540: the polyfill of `asyncDispose`.
Symbol.asyncDispose ??= Symbol('Symbol.asyncDispose');

export class RolldownBuild {
  #inputOptions: InputOptions;
  #bundler: BindingBundler;
  #bundlerImpl?: BundlerImplWithStopWorker;

  static asyncRuntimeShutdown = false;

  constructor(inputOptions: InputOptions) {
    this.#inputOptions = inputOptions;
    this.#bundler = new BindingBundler();
  }

  get closed(): boolean {
    return this.#bundlerImpl?.impl.closed ?? true;
  }

  // Create bundler for each `bundle.write/generate`
  async #getBundlerWithStopWorker(
    outputOptions: OutputOptions,
  ): Promise<BundlerImplWithStopWorker> {
    if (this.#bundlerImpl) {
      await this.#bundlerImpl.stopWorkers?.();
    }

    const option = await createBundlerOptions(
      this.#inputOptions,
      outputOptions,
      false,
    );

    if (RolldownBuild.asyncRuntimeShutdown) {
      startAsyncRuntime();
    }

    try {
      return this.#bundlerImpl = {
        impl: this.#bundler.createImpl(option.bundlerOptions),
        stopWorkers: option.stopWorkers,
        shutdown: () => {
          shutdownAsyncRuntime();
          RolldownBuild.asyncRuntimeShutdown = true;
        },
      };
    } catch (e) {
      await option.stopWorkers?.();
      throw e;
    }
  }

  async scan(): Promise<void> {
    const { impl } = await this.#getBundlerWithStopWorker({});
    const output = await impl.scan();
    return handleOutputErrors(output);
  }

  async generate(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    validateOption('output', outputOptions);
    const { impl } = await this.#getBundlerWithStopWorker(outputOptions);
    const output = await impl.generate();
    return transformToRollupOutput(output);
  }

  async write(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    validateOption('output', outputOptions);
    const { impl } = await this.#getBundlerWithStopWorker(outputOptions);
    const output = await impl.write();
    return transformToRollupOutput(output);
  }

  async close(): Promise<void> {
    if (this.#bundlerImpl) {
      await this.#bundlerImpl.stopWorkers?.();
      await this.#bundlerImpl.impl.close();
      this.#bundlerImpl.shutdown();
      this.#bundlerImpl = void 0;
    }
  }

  async [Symbol.asyncDispose](): Promise<void> {
    await this.close();
  }

  // TODO(shulaoda)
  // The `watchFiles` method returns a promise, but Rollup does not.
  // Converting it to a synchronous API might cause a deadlock if the user calls `write` and `watchFiles` simultaneously.
  get watchFiles(): Promise<string[]> {
    return this.#bundlerImpl?.impl.getWatchFiles() ?? Promise.resolve([]);
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>;
}
