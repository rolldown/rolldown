import { NormalizedInputOptions } from '../rollup-types'
import { InputOptions as BindingInputOptions } from '@rolldown/node-binding'
import path from 'path'
import { createBuildPluginAdapter } from './create-build-plugin-adapter'

export function createInputOptionsAdapter(
  options: NormalizedInputOptions,
): BindingInputOptions {
  return {
    input: normalizeInput(options.input),
    plugins: options.plugins.map((plugin) =>
      createBuildPluginAdapter(plugin, options),
    ),
    cwd: process.cwd(),
  }
}

function normalizeInput(
  input: NormalizedInputOptions['input'],
): BindingInputOptions['input'] {
  if (Array.isArray(input)) {
    return input.map((src) => {
      const name = path.parse(src).name
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
