import { NormalizedInputOptions } from '../rollup-types'
import { BindingInputOptions } from '../binding'
import nodePath from 'node:path'
import { bindingifyPlugin } from '../plugin/bindingify-plugin'
import { InputOptions, RolldownNormalizedInputOptions } from './input-options'

export function createInputOptionsAdapter(
  options: RolldownNormalizedInputOptions,
  inputOptions: InputOptions,
): BindingInputOptions {
  return {
    input: normalizeInput(options.input),
    plugins: options.plugins.map((plugin) =>
      // @ts-expect-error
      bindingifyPlugin(plugin, options),
    ),
    cwd: inputOptions.cwd ?? process.cwd(),
    external: inputOptions.external ? options.external : undefined,
    resolve: options.resolve,
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
