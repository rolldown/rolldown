import { BindingInputOptions } from '../binding'
import nodePath from 'node:path'
import { bindingifyPlugin } from '../plugin/bindingify-plugin'
import type { NormalizedInputOptions } from './normalized-input-options'
import { arraify } from '@src/utils'
import { NormalizedOutputOptions } from './normalized-output-options'

export function bindingifyInputOptions(
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
): BindingInputOptions {
  return {
    input: bindingifyInput(options.input),
    plugins: options.plugins.map((plugin) => {
      if ('_parallel' in plugin) {
        return undefined
      }
      return bindingifyPlugin(plugin, options, outputOptions)
    }),
    cwd: options.cwd ?? process.cwd(),
    external: options.external
      ? (function bindingifyExternal() {
          const external = options.external
          if (typeof external === 'function') {
            return (id, importer, isResolved) => {
              if (id.startsWith('\0')) return false
              return external(id, importer, isResolved) ?? false
            }
          }
          const externalArr = arraify(external)
          return (id, _importer, _isResolved) => {
            return externalArr.some((pat) => {
              if (pat instanceof RegExp) {
                return pat.test(id)
              }
              return id === pat
            })
          }
        })()
      : undefined,
    resolve: options.resolve
      ? (function bindingifyResolve() {
          const { alias, ...rest } = options.resolve

          return {
            alias: alias
              ? Object.entries(alias).map(([name, replacement]) => ({
                  find: name,
                  replacements: [replacement],
                }))
              : undefined,
            ...rest,
          }
        })()
      : undefined,
    platform: options.platform,
    shimMissingExports: options.shimMissingExports,
    // @ts-ignore TODO: logLevel shouldn't include `error`
    logLevel: options.logLevel,
  }
}

function bindingifyInput(
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
