import {
  type BundlerImplWithStopWorker,
  createBundlerImpl,
} from '../../utils/create-bundler';
import { transformToRollupOutput } from '../../utils/transform-to-rollup-output';

import type { BindingHmrUpdate } from '../../binding';
import { BindingBundler } from '../../binding';
import type { InputOptions } from '../../options/input-options';
import type { OutputOptions } from '../../options/output-options';
import type { HasProperty, TypeAssert } from '../../types/assert';
import type { RolldownOutput } from '../../types/rolldown-output';
import { normalizeErrors } from '../../utils/error';
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
    return this.#bundlerImpl?.impl.closed ?? true;
  }

  // Create bundler for each `bundle.write/generate`
  async #getBundlerWithStopWorker(
    outputOptions: OutputOptions,
  ): Promise<BundlerImplWithStopWorker> {
    if (this.#bundlerImpl) {
      await this.#bundlerImpl.stopWorkers?.();
    }
    return (this.#bundlerImpl = await createBundlerImpl(
      this.#bundler,
      this.#inputOptions,
      outputOptions,
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

  async generateHmrPatch(
    changedFiles: string[],
  ): Promise<BindingHmrUpdate[]> {
    const ret = await this.#bundlerImpl!.impl.generateHmrPatch(
      changedFiles,
    );
    switch (ret.type) {
      case 'Ok':
        return ret.field0;
      case 'Error':
        throw normalizeErrors(ret.field0);
      default:
        throw new Error('Unknown error');
    }
  }

  async hmrInvalidate(
    file: string,
    firstInvalidatedBy?: string,
  ): Promise<BindingHmrUpdate> {
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
