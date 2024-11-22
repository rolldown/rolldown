import { bindingifyPlugin } from '../plugin/bindingify-plugin'
import { arraify, unsupported } from '../utils/misc'
import { BindingLogLevel } from '../binding'
import {
  bindingifyBuiltInPlugin,
  BuiltinPlugin,
} from '../plugin/builtin-plugin'
import { PluginContextData } from '../plugin/plugin-context-data'
import { normalizedStringOrRegex } from '../utils/normalize-string-or-regex'
import type { NormalizedInputOptions } from './normalized-input-options'
import type { NormalizedOutputOptions } from './normalized-output-options'
import type {
  BindingWatchOption,
  BindingInputOptions,
  BindingInjectImportNamed,
  BindingInjectImportNamespace,
} from '../binding'
import { RolldownPlugin } from '..'

export function bindingifyInputOptions(
  inputOptions: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
  plugins: RolldownPlugin[],
): BindingInputOptions {
  const pluginContextData = new PluginContextData()
  return {
    input: bindingifyInput(inputOptions.input),
    plugins: plugins.map((plugin) => {
      if ('_parallel' in plugin) {
        return undefined
      }
      if (plugin instanceof BuiltinPlugin) {
        return bindingifyBuiltInPlugin(plugin)
      }
      return bindingifyPlugin(
        plugin,
        inputOptions,
        outputOptions,
        pluginContextData,
      )
    }),
    cwd: inputOptions.cwd ?? process.cwd(),
    external: inputOptions.external
      ? (function bindingifyExternal() {
          const external = inputOptions.external
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
    resolve: inputOptions.resolve
      ? (function bindingifyResolve() {
          const { alias, extensionAlias, ...rest } = inputOptions.resolve

          return {
            alias: alias
              ? Object.entries(alias).map(([name, replacement]) => ({
                  find: name,
                  replacements: [replacement],
                }))
              : undefined,
            extensionAlias: extensionAlias
              ? Object.entries(extensionAlias).map(([name, value]) => ({
                  target: name,
                  replacements: value,
                }))
              : undefined,
            ...rest,
          }
        })()
      : undefined,
    platform: inputOptions.platform,
    shimMissingExports: inputOptions.shimMissingExports,
    // @ts-ignore TODO The typing should import from binding
    logLevel: bindingifyLogLevel(inputOptions.logLevel),
    onLog: (level, log) => {
      inputOptions.onLog(level, { code: log.code, message: log.message })
    },
    // After normalized, `false` will be converted to `undefined`, otherwise, default value will be assigned
    // Because it is hard to represent Enum in napi, ref: https://github.com/napi-rs/napi-rs/issues/507
    // So we use `undefined | NormalizedTreeshakingOptions` (or Option<NormalizedTreeshakingOptions> in rust side), to represent `false | NormalizedTreeshakingOptions`
    treeshake: bindingifyTreeshakeOptions(inputOptions.treeshake),
    moduleTypes: inputOptions.moduleTypes,
    define: inputOptions.define
      ? Object.entries(inputOptions.define)
      : undefined,
    inject: inputOptions.inject
      ? Object.entries(inputOptions.inject).map(
          ([alias, item]):
            | BindingInjectImportNamed
            | BindingInjectImportNamespace => {
            if (Array.isArray(item)) {
              // import * as fs from 'node:fs'
              // fs: ['node:fs', '*' ],
              if (item[1] === '*') {
                return {
                  tagNamespace: true,
                  alias,
                  from: item[0],
                }
              }

              // import { Promise } from 'es6-promise'
              // Promise: [ 'es6-promise', 'Promise' ],

              // import { Promise as P } from 'es6-promise'
              // P: [ 'es6-promise', 'Promise' ],
              return {
                tagNamed: true,
                alias,
                from: item[0],
                imported: item[1],
              }
            } else {
              // import $ from 'jquery'
              // $: 'jquery',

              // 'Object.assign': path.resolve( 'src/helpers/object-assign.js' ),
              return {
                tagNamed: true,
                imported: 'default',
                alias,
                from: item,
              }
            }
          },
        )
      : undefined,
    experimental: {
      strictExecutionOrder: inputOptions.experimental?.strictExecutionOrder,
      disableLiveBindings: inputOptions.experimental?.disableLiveBindings,
      viteMode: inputOptions.experimental?.viteMode,
    },
    profilerNames: inputOptions?.profilerNames,
    jsx: bindingifyJsx(inputOptions.jsx),
    watch: bindingifyWatch(inputOptions.watch),
    dropLabels: inputOptions.dropLabels,
  }
}

function bindingifyLogLevel(
  logLevel?: NormalizedInputOptions['logLevel'],
): BindingInputOptions['logLevel'] {
  switch (logLevel) {
    case 'silent':
      return BindingLogLevel.Silent
    case 'warn':
      return BindingLogLevel.Warn
    case 'info':
    case undefined:
      return BindingLogLevel.Info
    case 'debug':
      return BindingLogLevel.Debug
    default:
      return undefined
  }
}

function bindingifyInput(
  input: NormalizedInputOptions['input'],
): BindingInputOptions['input'] {
  if (input === undefined) {
    return []
  }

  if (typeof input === 'string') {
    return [{ import: input }]
  }

  if (Array.isArray(input)) {
    return input.map((src) => ({ import: src }))
  }

  return Object.entries(input).map((value) => {
    return { name: value[0], import: value[1] }
  })
}

function bindingifyJsx(
  input: NormalizedInputOptions['jsx'],
): BindingInputOptions['jsx'] {
  if (input) {
    const mode = input.mode ?? 'classic'
    return {
      runtime: mode,
      importSource:
        mode === 'classic'
          ? input.importSource
          : mode === 'automatic'
            ? input.jsxImportSource
            : undefined,
      pragma: input.factory,
      pragmaFrag: input.fragment,
      development: input.development,
      refresh: input.refresh,
    }
  }
}

function bindingifyWatch(
  watch: NormalizedInputOptions['watch'],
): BindingInputOptions['watch'] {
  if (watch) {
    let value = {
      skipWrite: watch.skipWrite,
      include: normalizedStringOrRegex(watch.include),
      exclude: normalizedStringOrRegex(watch.exclude),
    } as BindingWatchOption
    if (watch.notify) {
      value.notify = {
        pollInterval: watch.notify.pollInterval,
        compareContents: watch.notify.compareContents,
      }
    }
    if (watch.chokidar) {
      unsupported(
        'The watch chokidar option is deprecated, please use notify options instead of it.',
      )
    }
    return value
  }
}

function bindingifyTreeshakeOptions(
  config: NormalizedInputOptions['treeshake'],
): BindingInputOptions['treeshake'] {
  if (config === false) {
    return undefined
  }
  if (config === true || config === undefined) {
    return {
      moduleSideEffects: true,
      annotations: true,
    }
  }
  let normalizedConfig: BindingInputOptions['treeshake'] = {
    moduleSideEffects: true,
  }
  if (config.moduleSideEffects === undefined) {
    normalizedConfig.moduleSideEffects = true
  } else if (config.moduleSideEffects === 'no-external') {
    normalizedConfig.moduleSideEffects = [
      { external: true, sideEffects: false },
      { external: false, sideEffects: true },
    ]
  } else {
    normalizedConfig.moduleSideEffects = config.moduleSideEffects
  }

  normalizedConfig.annotations = config.annotations ?? true
  return normalizedConfig
}
