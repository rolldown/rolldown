import {
  type BundlerImplWithStopWorker,
  createBundlerImpl,
} from '../../utils/create-bundler';
import { transformToRollupOutput } from '../../utils/transform-to-rollup-output';

import type { BindingHmrOutputPatch } from '../../binding';
import { BindingBundler } from '../../binding';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import type { HasProperty, TypeAssert } from '../../types/assert';
import type { RolldownOutput } from '../../types/rolldown-output';
import { transformHmrPatchOutput } from '../../utils/transform-hmr-patch-output';
import { validateOption } from '../../utils/validator';

// @ts-expect-error TS2540: the polyfill of `asyncDispose`.
Symbol.asyncDispose ??= Symbol('Symbol.asyncDispose');

export class RolldownBuild {
  #inputOptions: InputOptions;
  #bundler: BindingBundler;
  #bundlerImpl?: BundlerImplWithStopWorker;

  constructor(inputOptions: InputOptions) {
    // TODO: Check if `inputOptions.output` is set. If so, throw an warning that it is ignored.
    this.#inputOptions = inputOptions;
    this.#bundler = new BindingBundler();
  }

  get closed(): boolean {
    // If the bundler has not yet been created, it is not closed.
    return this.#bundlerImpl?.impl.closed ?? false;
  }

  // Create bundler for each `bundle.write/generate`
  async #getBundlerWithStopWorker(
    outputOptions: OutputOptions,
    isClose?: boolean,
  ): Promise<BundlerImplWithStopWorker> {
    if (this.#bundlerImpl) {
      await this.#bundlerImpl.stopWorkers?.();
    }
    return (this.#bundlerImpl = await createBundlerImpl(
      this.#bundler,
      this.#inputOptions,
      outputOptions,
      isClose,
    ));
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
    // Create new one bundler to run `closeBundle` hook, here using `isClose` flag to avoid call `outputOptions` hook.
    const { impl, stopWorkers, shutdown } = await this
      .#getBundlerWithStopWorker(
        {},
        true,
      );
    await stopWorkers?.();
    await impl.close();
    shutdown();
  }

  async [Symbol.asyncDispose](): Promise<void> {
    await this.close();
  }

  async generateHmrPatch(
    changedFiles: string[],
  ): Promise<BindingHmrOutputPatch | undefined> {
    const output = await this.#bundlerImpl!.impl.generateHmrPatch(
      changedFiles,
    );
    return transformHmrPatchOutput(output);
  }

  async hmrInvalidate(
    file: string,
    firstInvalidatedBy?: string,
  ): Promise<BindingHmrOutputPatch> {
    const output = await this.#bundlerImpl!.impl.hmrInvalidate(
      file,
      firstInvalidatedBy,
    );
    return transformHmrPatchOutput(output)!;
  }

  // TODO(underfin)
  // The `watchFiles` method returns a promise, but Rollup does not.
  // Converting it to a synchronous API might cause a deadlock if the user calls `write` and `watchFiles` simultaneously.
  get watchFiles(): Promise<string[]> {
    return this.#bundlerImpl?.impl.getWatchFiles() ?? Promise.resolve([]);
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>;
}
