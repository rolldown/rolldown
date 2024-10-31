import type { OutputOptions } from './options/output-options'
import { transformToRollupOutput } from './utils/transform-to-rollup-output'
import { BundlerWithStopWorker, createBundler } from './utils/create-bundler'

import type { RolldownOutput } from './types/rolldown-output'
import type { HasProperty, TypeAssert } from './utils/type-assert'
import type { InputOptions } from './options/input-options'

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
      return this.#bundler
    }
    return createBundler(this.#inputOptions, outputOptions)
  }

  async generate(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    const bundler = await createBundler(this.#inputOptions, outputOptions)
    const output = await bundler.bundler.generate()
    await bundler.stopWorkers?.()
    await bundler.bundler.close()
    return transformToRollupOutput(output)
  }

  async write(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    const bundler = await createBundler(this.#inputOptions, outputOptions)
    const output = await bundler.bundler.write()
    await bundler.stopWorkers?.()
    await bundler.bundler.close()
    return transformToRollupOutput(output)
  }

  async close(): Promise<void> {
    const bundler = await this.#getBundlerWithStopWorker({})
    await bundler.stopWorkers?.()
    await bundler.bundler.close()
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>
}
