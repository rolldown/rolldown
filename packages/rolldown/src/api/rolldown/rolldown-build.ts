import {
  type BundlerWithStopWorker,
  createBundler,
} from '../../utils/create-bundler';
import { transformToRollupOutput } from '../../utils/transform-to-rollup-output';

import type { BindingHmrOutput } from '../../binding';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import type { HasProperty, TypeAssert } from '../../types/assert';
import type { RolldownOutput } from '../../types/rolldown-output';
import { validateOption } from '../../utils/validator';

// @ts-expect-error TS2540: the polyfill of `asyncDispose`.
Symbol.asyncDispose ??= Symbol('Symbol.asyncDispose');

export class RolldownBuild {
  #inputOptions: InputOptions;
  #bundler?: BundlerWithStopWorker;

  constructor(inputOptions: InputOptions) {
    // TODO: Check if `inputOptions.output` is set. If so, throw an warning that it is ignored.
    this.#inputOptions = inputOptions;
  }

  get closed(): boolean {
    // If the bundler has not yet been created, it is not closed.
    return this.#bundler?.bundler.closed ?? false;
  }

  // Create bundler for each `bundle.write/generate`
  async #getBundlerWithStopWorker(
    outputOptions: OutputOptions,
    isClose?: boolean,
  ): Promise<BundlerWithStopWorker> {
    if (this.#bundler) {
      await this.#bundler.stopWorkers?.();
    }
    return (this.#bundler = await createBundler(
      this.#inputOptions,
      outputOptions,
      isClose,
    ));
  }

  async generate(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    validateOption('output', outputOptions);
    const { bundler } = await this.#getBundlerWithStopWorker(outputOptions);
    const output = await bundler.generate();
    return transformToRollupOutput(output);
  }

  async write(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    validateOption('output', outputOptions);
    const { bundler } = await this.#getBundlerWithStopWorker(outputOptions);
    const output = await bundler.write();
    return transformToRollupOutput(output);
  }

  async close(): Promise<void> {
    // Create new one bundler to run `closeBundle` hook, here using `isClose` flag to avoid call `outputOptions` hook.
    const { bundler, stopWorkers, shutdown } = await this
      .#getBundlerWithStopWorker({}, true);
    await stopWorkers?.();
    await bundler.close();
    shutdown();
  }

  async [Symbol.asyncDispose](): Promise<void> {
    await this.close();
  }

  async generateHmrPatch(
    changedFiles: string[],
  ): Promise<BindingHmrOutput | undefined> {
    return this.#bundler?.bundler.generateHmrPatch(changedFiles);
  }

  async hmrInvalidate(
    file: string,
    firstInvalidatedBy?: string,
  ): Promise<BindingHmrOutput | undefined> {
    return this.#bundler?.bundler.hmrInvalidate(file, firstInvalidatedBy);
  }

  get watchFiles(): string[] {
    return this.#bundler?.bundler.watchFiles ?? [];
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>;
}
