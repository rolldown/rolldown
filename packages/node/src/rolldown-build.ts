import { Bundler } from '@rolldown/node-binding'
import { normalizeOutputOptions, OutputOptions } from './options/output-options'
import type { RollupBuild, SerializedTimings } from './rollup-types'
import {
  transformToRollupOutput,
  RolldownOutput,
  unimplemented,
  normalizePluginOption,
} from './utils'
import { createBuildPluginAdapter } from './options/create-build-plugin-adapter'

export class RolldownBuild implements RollupBuild {
  #bundler: Bundler
  constructor(bundler: Bundler) {
    this.#bundler = bundler
  }

  closed = false

  // @ts-expect-error 2416
  async generate(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
    return await bundle(this.#bundler, outputOptions, false)
  }

  // @ts-expect-error 2416
  async write(outputOptions?: OutputOptions = {}): Promise<RolldownOutput> {
    return await bundle(this.#bundler, outputOptions, true)
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

let buildIndex = 0

async function bundle(
  bundler: Bundler,
  outputOptions: OutputOptions,
  write: boolean,
) {
  const index = buildIndex
  buildIndex++

  const bindingOptions = normalizeOutputOptions(outputOptions)

  if (outputOptions.plugins) {
    const outputPlugins = (
      await normalizePluginOption(outputOptions.plugins)
    ).map((plugin) => createBuildPluginAdapter(plugin))
    bundler.setOutputPlugins(index, outputPlugins)
  }

  if (write) {
    const output = await bundler.write(index, bindingOptions)
    return transformToRollupOutput(output)
  } else {
    const output = await bundler.generate(index, bindingOptions)
    return transformToRollupOutput(output)
  }
}
