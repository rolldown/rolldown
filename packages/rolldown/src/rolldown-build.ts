import { transformToRollupOutput } from './utils/transform-to-rollup-output'
import { BundlerWithStopWorker, createBundler } from './utils/create-bundler'

import type { InputOptions } from './types/input-options'
import type { OutputOptions } from './types/output-options'
import type { RolldownOutput } from './types/rolldown-output'
import type { HasProperty, TypeAssert } from './types/assert'

export class RolldownBuild {
  #inputOptions: InputOptions
  #bundler?: BundlerWithStopWorker

  constructor(inputOptions: InputOptions) {
    // TODO: Check if `inputOptions.output` is set. If so, throw an warning that it is ignored.
    this.#inputOptions = inputOptions
  }

  // Create bundler for each `bundle.write/generate`
  async #getBundlerWithStopWorker(
    outputOptions: OutputOptions,
  ): Promise<BundlerWithStopWorker> {
    if (this.#bundler) {
      this.#bundler.stopWorkers?.()
    }
    return (this.#bundler = await createBundler(
      this.#inputOptions,
      outputOptions,
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
    const { bundler, stopWorkers } = await this.#getBundlerWithStopWorker({})
    await stopWorkers?.()
    await bundler.close()
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>
}
