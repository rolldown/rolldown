import { NormalizedInputOptions } from '../rollup-types'
import { BindingInputOptions } from '../binding'
import nodePath from 'node:path'
import { bindingifyPlugin } from '../plugin/bindingify-plugin'
import { InputOptions, RolldownNormalizedInputOptions } from './input-options'
import { NormalizedOutputOptions } from './output-options'

export function createInputOptionsAdapter(
  options: RolldownNormalizedInputOptions,
  inputOptions: InputOptions,
  outputOptions: NormalizedOutputOptions,
): BindingInputOptions {
  return {
    input: normalizeInput(options.input),
    plugins: options.plugins.map((plugin) => {
      if ('_parallel' in plugin) {
        return undefined
      }
      return bindingifyPlugin(plugin, options, outputOptions)
    }),
    cwd: inputOptions.cwd ?? process.cwd(),
    external: inputOptions.external ? options.external : undefined,
    resolve: options.resolve,
    platform: options.platform,
    shimMissingExports: options.shimMissingExports,
    logLevel: inputOptions.logLevel,
    warmupFiles: options.warmupFiles,
    warmupFilesExclude: options.warmupFilesExclude,
  }
}

function normalizeInput(
  input: NormalizedInputOptions['input'],
): BindingInputOptions['input'] {
  if (Array.isArray(input)) {
    return input.map((src) => {
      const name = nodePath.parse(src).name
      return {
        name,
        import: src,
      }
    })
  } else {
    return Object.entries(input).map((value) => {
      return { name: value[0], import: value[1] }
    })
  }
}
