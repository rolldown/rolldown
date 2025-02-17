import { transformToRollupOutput } from '../../utils/transform-to-rollup-output'
import {
  BundlerWithStopWorker,
  createBundler,
} from '../../utils/create-bundler'

import type { InputOptions } from '../../options/input-options'
import type { OutputOptions } from '../../options/output-options'
import type { RolldownOutput } from '../../types/rolldown-output'
import type { HasProperty, TypeAssert } from '../../types/assert'

// @ts-expect-error TS2540: the polyfill of `asyncDispose`.
Symbol.asyncDispose ??= Symbol('Symbol.asyncDispose')

export class RolldownBuild {
  #inputOptions: InputOptions
  #bundler?: BundlerWithStopWorker

  constructor(inputOptions: InputOptions) {
    // TODO: Check if `inputOptions.output` is set. If so, throw an warning that it is ignored.
    this.#inputOptions = inputOptions
  }

  get closed(): boolean {
    // If the bundler has not yet been created, it is not closed.
    return this.#bundler ? this.#bundler.bundler.closed : false
  }

  // Create bundler for each `bundle.write/generate`
  async #getBundlerWithStopWorker(
    outputOptions: OutputOptions,
    isClose?: boolean,
  ): Promise<BundlerWithStopWorker> {
    if (this.#bundler) {
      this.#bundler.stopWorkers?.()
    }
    return (this.#bundler = await createBundler(
      this.#inputOptions,
      outputOptions,
      isClose,
    ))
  }

  async generate(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    const { bundler } = await this.#getBundlerWithStopWorker(outputOptions)
    const output = await bundler.generate()
    return transformToRollupOutput(output)
  }

  async write(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    const { bundler } = await this.#getBundlerWithStopWorker(outputOptions)
    const output = await bundler.write()
    return transformToRollupOutput(output)
  }

  async close(): Promise<void> {
    // Create new one bundler to run `closeBundle` hook, here using `isClose` flag to avoid call `outputOptions` hook.
    const { bundler, stopWorkers } = await this.#getBundlerWithStopWorker(
      {},
      true,
    )
    await stopWorkers?.()
    await bundler.close()
  }

  async [Symbol.asyncDispose](): Promise<void> {
    await this.close()
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>
}
