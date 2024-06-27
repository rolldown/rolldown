import { BindingInputOptions, BindingLogLevel } from '../binding'
import nodePath from 'node:path'
import { bindingifyPlugin } from '../plugin/bindingify-plugin'
import type { NormalizedInputOptions } from './normalized-input-options'
import { arraify } from '../utils/misc'
import type { NormalizedOutputOptions } from './normalized-output-options'
import type { LogLevelOption } from '../log/logging'
import {
  bindingifyBuiltInPlugin,
  BuiltinPlugin,
} from '../plugin/bindingify-builtin-plugin'

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
      if (plugin instanceof BuiltinPlugin) {
        return bindingifyBuiltInPlugin(plugin)
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
    // @ts-ignore TODO The typing should import from binding
    logLevel: bindingifyLogLevel(options.logLevel),
    onLog: (level, log) => {
      options.onLog(level, { code: log.code, message: log.message })
    },
    treeshake: options.treeshake,
    moduleTypes: options.moduleTypes,
  }
}

function bindingifyLogLevel(
  logLevel: LogLevelOption,
): BindingLogLevel | undefined {
  switch (logLevel) {
    case 'silent':
      return BindingLogLevel.Silent
    case 'warn':
      return BindingLogLevel.Warn
    case 'info':
      return BindingLogLevel.Info
    case 'debug':
      return BindingLogLevel.Debug

    default:
      throw new Error(`Unexpected log level: ${logLevel}`)
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
