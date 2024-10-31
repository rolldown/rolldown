import { Bundler } from './binding'
import type { OutputOptions } from './options/output-options'
import { transformToRollupOutput } from './utils/transform-to-rollup-output'
import { BundlerWithStopWorkers, createBundler } from './utils/create-bundler'

import type { RolldownOutput } from './types/rolldown-output'
import type { HasProperty, TypeAssert } from './utils/type-assert'
import type { InputOptions } from './options/input-options'

export class RolldownBuild {
  #inputOptions: InputOptions
  #bundlers: BundlerWithStopWorkers[] = []

  constructor(inputOptions: InputOptions) {
    // TODO: Check if `inputOptions.output` is set. If so, throw an warning that it is ignored.
    this.#inputOptions = inputOptions
  }

  // Create bundler for each `bundle.write/generate`
  async #getBundler(outputOptions: OutputOptions): Promise<Bundler> {
    const { bundler, stopWorkers } = await createBundler(
      this.#inputOptions,
      outputOptions,
    )
    this.#bundlers.push({ bundler, stopWorkers })
    return bundler
  }

  async generate(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    const bundler = await this.#getBundler(outputOptions)
    const output = await bundler.generate()
    return transformToRollupOutput(output)
  }

  async write(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    const bundler = await this.#getBundler(outputOptions)
    const output = await bundler.write()
    return transformToRollupOutput(output)
  }

  async close(): Promise<void> {
    // If the bundler not create, create one to make close related things(eg. hooks) could be work.
    if (this.#bundlers.length === 0) {
      await this.#getBundler({})
    }
    // Different with rollup: her need to close all bundlers.
    for (const { bundler, stopWorkers } of this.#bundlers) {
      await stopWorkers?.()
      await bundler.close()
    }
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>
}
