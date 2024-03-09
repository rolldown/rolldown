import { Bundler } from '@rolldown/node-binding'
import { normalizeOutputOptions, OutputOptions } from './options/output-options'
import { transformToRollupOutput, unimplemented } from './utils'
import { RolldownOutput } from './objects/rolldown-output'
import { HasProperty, TypeAssert } from './utils/type-assert'

export class RolldownBuild {
  #bundler: Bundler
  constructor(bundler: Bundler) {
    this.#bundler = bundler
  }

  async generate(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    const bindingOptions = normalizeOutputOptions(outputOptions)
    const output = await this.#bundler.write(bindingOptions)
    return transformToRollupOutput(output) as RolldownOutput
  }

  async write(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    const bindingOptions = normalizeOutputOptions(outputOptions)
    const output = await this.#bundler.write(bindingOptions)
    return transformToRollupOutput(output) as RolldownOutput
  }
}

function _assert() {
  type _ = TypeAssert<HasProperty<RolldownBuild, 'generate' | 'write'>>
}
