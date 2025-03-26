import { BindingLogLevel } from '../binding'
import { bindingifyPlugin } from '../plugin/bindingify-plugin'
import { PluginContextData } from '../plugin/plugin-context-data'
import { bindingifyBuiltInPlugin } from '../builtin-plugin/utils'
import { BuiltinPlugin } from '../builtin-plugin/constructors'
import { arraify } from './misc'
import { normalizedStringOrRegex } from './normalize-string-or-regex'
import type { RolldownPlugin } from '../plugin'
import type { InputOptions } from '../options/input-options'
import type { OutputOptions } from '../options/output-options'
import type {
  BindingInputOptions,
  BindingInjectImportNamed,
  BindingInjectImportNamespace,
  BindingDeferSyncScanData,
} from '../binding'
import { LogHandler } from '../types/misc'
import { LogLevelOption } from '../log/logging'
import { bindingifySideEffects } from './transform-side-effects'

export function bindingifyInputOptions(
  rawPlugins: RolldownPlugin[],
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  normalizedOutputPlugins: RolldownPlugin[],
  onLog: LogHandler,
  logLevel: LogLevelOption,
  watchMode: boolean,
): BindingInputOptions {
  const pluginContextData = new PluginContextData()

  const plugins = rawPlugins.map((plugin) => {
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
      normalizedOutputPlugins,
      onLog,
      logLevel,
      watchMode,
    )
  })

  return {
    input: bindingifyInput(inputOptions.input),
    plugins,
    cwd: inputOptions.cwd ?? process.cwd(),
    external: bindingifyExternal(inputOptions.external),
    resolve: bindingifyResolve(inputOptions.resolve),
    platform: inputOptions.platform,
    shimMissingExports: inputOptions.shimMissingExports,
    logLevel: bindingifyLogLevel(logLevel),
    onLog,
    // After normalized, `false` will be converted to `undefined`, otherwise, default value will be assigned
    // Because it is hard to represent Enum in napi, ref: https://github.com/napi-rs/napi-rs/issues/507
    // So we use `undefined | NormalizedTreeshakingOptions` (or Option<NormalizedTreeshakingOptions> in rust side), to represent `false | NormalizedTreeshakingOptions`
    treeshake: bindingifyTreeshakeOptions(inputOptions.treeshake),
    moduleTypes: inputOptions.moduleTypes,
    define: inputOptions.define
      ? Object.entries(inputOptions.define)
      : undefined,
    inject: bindingifyInject(inputOptions.inject),
    experimental: {
      strictExecutionOrder: inputOptions.experimental?.strictExecutionOrder,
      disableLiveBindings: inputOptions.experimental?.disableLiveBindings,
      viteMode: inputOptions.experimental?.viteMode,
      resolveNewUrlToAsset: inputOptions.experimental?.resolveNewUrlToAsset,
      hmr: inputOptions.experimental?.hmr,
    },
    profilerNames: inputOptions?.profilerNames,
    jsx: bindingifyJsx(inputOptions.jsx),
    transform: inputOptions.transform,
    watch: bindingifyWatch(inputOptions.watch),
    dropLabels: inputOptions.dropLabels,
    keepNames: inputOptions.keepNames,
    checks: inputOptions.checks,
    deferSyncScanData: () => {
      let ret: BindingDeferSyncScanData[] = []
      pluginContextData.moduleOptionMap.forEach((value, key) => {
        if (value.invalidate) {
          ret.push({
            id: key,
            sideEffects: bindingifySideEffects(value.moduleSideEffects),
          })
        }
      })
      return ret
    },
    makeAbsoluteExternalsRelative: bindingifyMakeAbsoluteExternalsRelative(
      inputOptions.makeAbsoluteExternalsRelative,
    ),
  }
}

function bindingifyExternal(
  external: InputOptions['external'],
): BindingInputOptions['external'] {
  if (external) {
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
  }
}

function bindingifyResolve(
  resolve: InputOptions['resolve'],
): BindingInputOptions['resolve'] {
  if (resolve) {
    const { alias, extensionAlias, ...rest } = resolve

    return {
      alias: alias
        ? Object.entries(alias).map(([name, replacement]) => ({
            find: name,
            replacements: arraify(replacement),
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
  }
}

function bindingifyInject(
  inject: InputOptions['inject'],
): BindingInputOptions['inject'] {
  if (inject) {
    return Object.entries(inject).map(
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
  }
}

function bindingifyLogLevel(
  logLevel: InputOptions['logLevel'],
): BindingInputOptions['logLevel'] {
  switch (logLevel) {
    case 'silent':
      return BindingLogLevel.Silent
    case 'debug':
      return BindingLogLevel.Debug
    case 'warn':
      return BindingLogLevel.Warn
    case 'info':
      return BindingLogLevel.Info
    default:
      throw new Error(`Unexpected log level: ${logLevel}`)
  }
}

function bindingifyInput(
  input: InputOptions['input'],
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

  return Object.entries(input).map(([name, import_path]) => {
    return { name, import: import_path }
  })
}

// The `automatic` is most user usages, so it is different rollup's default value `false`
function bindingifyJsx(input: InputOptions['jsx']): BindingInputOptions['jsx'] {
  switch (input) {
    case false:
      return { type: 'Disable' }
    case 'react':
      return { type: 'React' }
    case 'react-jsx':
      return { type: 'ReactJsx' }
    case 'preserve':
      return { type: 'Preserve' }
    case undefined:
      return undefined
  }
  if (input.mode === 'preserve') {
    return { type: 'Preserve' }
  }
  const mode = input.mode ?? 'automatic'
  return {
    type: 'Enable',
    field0: {
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
    },
  }
}

function bindingifyWatch(
  watch: InputOptions['watch'],
): BindingInputOptions['watch'] {
  if (watch) {
    return {
      buildDelay: watch.buildDelay,
      skipWrite: watch.skipWrite,
      include: normalizedStringOrRegex(watch.include),
      exclude: normalizedStringOrRegex(watch.exclude),
    }
  }
}

function bindingifyTreeshakeOptions(
  config: InputOptions['treeshake'],
): BindingInputOptions['treeshake'] {
  if (config === false) {
    return undefined
  }

  if (config === true || config === undefined) {
    return {
      moduleSideEffects: true,
    }
  }

  let normalizedConfig: BindingInputOptions['treeshake'] = {
    moduleSideEffects: true,
    annotations: config.annotations,
    manualPureFunctions: config.manualPureFunctions,
    unknownGlobalSideEffects: config.unknownGlobalSideEffects,
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

  return normalizedConfig
}

function bindingifyMakeAbsoluteExternalsRelative(
  makeAbsoluteExternalsRelative: InputOptions['makeAbsoluteExternalsRelative'],
): BindingInputOptions['makeAbsoluteExternalsRelative'] {
  if (makeAbsoluteExternalsRelative === 'ifRelativeSource') {
    return { type: 'IfRelativeSource' }
  }
  if (typeof makeAbsoluteExternalsRelative === 'boolean') {
    return { type: 'Bool', field0: makeAbsoluteExternalsRelative }
  }
}
