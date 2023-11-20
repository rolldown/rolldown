import { Bundler } from '@rolldown/node-binding'
import { InputOptions, normalizeInputOptions } from './options/input-options'
import { normalizeOutputOptions, OutputOptions } from './options/output-options'
import type { RollupBuild, SerializedTimings } from './rollup-types'
import { transformToRollupOutput, RolldownOutput, unimplemented } from './utils'
import { createInputOptionsAdapter } from './options/input-options-adapter'

export class RolldownBuild implements RollupBuild {
  #bundler: Bundler
  private constructor(bundler: Bundler) {
    this.#bundler = bundler
  }

  static async createBundler(inputOptions: InputOptions): Promise<Bundler> {
    // Convert `InputOptions` to `NormalizedInputOptions`.
    const normalizedInputOptions = await normalizeInputOptions(inputOptions)
    // Convert `NormalizedInputOptions` to `BindingInputOptions`
    const bindingInputOptions = createInputOptionsAdapter(
      normalizedInputOptions,
      inputOptions,
    )
    return new Bundler(bindingInputOptions)
  }

  static async fromInputOptionsForScanStage(
    inputOptions: InputOptions,
  ): Promise<void> {
    const bundler = await RolldownBuild.createBundler(inputOptions)
    await bundler.scan()
  }

  static async fromInputOptions(
    inputOptions: InputOptions,
  ): Promise<RolldownBuild> {
    const bundler = await RolldownBuild.createBundler(inputOptions)
    await bundler.build()
    return new RolldownBuild(bundler)
  }

  closed = false

  // @ts-expect-error 2416
  async generate(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    const bindingOptions = normalizeOutputOptions(outputOptions)
    const output = await this.#bundler.write(bindingOptions)
    return transformToRollupOutput(output)
  }

  // @ts-expect-error 2416
  async write(outputOptions?: OutputOptions = {}): Promise<RolldownOutput> {
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
