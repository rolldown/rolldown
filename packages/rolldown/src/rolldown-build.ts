import { Bundler } from './binding'
import type { OutputOptions } from './options/output-options'
import { createBundler, transformToRollupOutput } from './utils'
import type { RolldownOutput } from './types/rolldown-output'
import type { HasProperty, TypeAssert } from './utils/type-assert'
import type { InputOptions } from './options/input-options'

export class RolldownBuild {
  #inputOptions: InputOptions
  #bundler?: Bundler
  #stopWorkers?: () => Promise<void>

  constructor(inputOptions: InputOptions) {
    // TODO: Check if `inputOptions.output` is set. If so, throw an warning that it is ignored.
    this.#inputOptions = inputOptions
  }

  async #getBundler(outputOptions: OutputOptions): Promise<Bundler> {
    if (typeof this.#bundler === 'undefined') {
      const { bundler, stopWorkers } = await createBundler(
        this.#inputOptions,
        outputOptions,
      )
      this.#bundler = bundler
      this.#stopWorkers = stopWorkers
    }
    return this.#bundler
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

  async destroy(): Promise<void> {
    await this.#stopWorkers?.()
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>
}
