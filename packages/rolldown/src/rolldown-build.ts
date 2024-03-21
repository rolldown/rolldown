import { Bundler } from './binding'
import { normalizeOutputOptions, OutputOptions } from './options/output-options'
import { createBundler, transformToRollupOutput, unimplemented } from './utils'
import { RolldownOutput } from './objects/rolldown-output'
import { HasProperty, TypeAssert } from './utils/type-assert'
import { InputOptions } from './options/input-options'

export class RolldownBuild {
  #inputOptions: InputOptions
  #bundler?: Bundler

  constructor(inputOptions: InputOptions) {
    // TODO: Check if `inputOptions.output` is set. If so, throw an warning that it is ignored.
    this.#inputOptions = inputOptions
  }

  async #getBundler(outputOptions: OutputOptions): Promise<Bundler> {
    if (typeof this.#bundler === 'undefined') {
      this.#bundler = await createBundler(this.#inputOptions, outputOptions)
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
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>
}
