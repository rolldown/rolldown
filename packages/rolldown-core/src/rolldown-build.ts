import {
  Bundler,
  type InputOptions as BindingInputOptions,
} from '@rolldown/node-binding'
import { InputOptions, normalizeInputOptions } from './options/input-options'
import { normalizeOutputOptions, OutputOptions } from './options/output-options'
import type {
  RollupBuild,
  RollupOutput,
  SerializedTimings,
} from './rollup-types'
import { transformToRollupOutput, unimplemented } from './utils'

export class RolldownBuild implements RollupBuild {
  #bundler: Bundler
  private constructor(bundler: Bundler) {
    this.#bundler = bundler
  }

  static async fromInputOptions(
    inputOptions: InputOptions,
  ): Promise<RolldownBuild> {
    const bindingOptions: BindingInputOptions = await normalizeInputOptions(
      inputOptions,
    )
    const bundler = new Bundler(bindingOptions)
    return new RolldownBuild(bundler)
  }

  closed = false

  // @ts-expect-error 2416
  async generate(outputOptions: OutputOptions): Promise<RollupOutput> {
    const bindingOptions = normalizeOutputOptions(outputOptions)
    const output = await this.#bundler.write(bindingOptions)
    return transformToRollupOutput(output)
  }

  // @ts-expect-error 2416
  async write(outputOptions: OutputOptions): Promise<RollupOutput> {
    const bindingOptions = normalizeOutputOptions(outputOptions)
    const output = await this.#bundler.write(bindingOptions)
    return transformToRollupOutput(output)
  }

  async close() {
    this.closed = true
  }

  // -- unimplemented

  get cache(): undefined {
    throw unimplemented()
    return unimplemented()
  }
  get watchFiles(): string[] {
    throw unimplemented()
    return unimplemented()
  }
  get getTimings(): () => SerializedTimings {
    throw unimplemented()
    return unimplemented()
  }
}
