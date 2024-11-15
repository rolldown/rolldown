import {
  normalizeEcmaTransformPluginConfig,
  TransformPluginConfig,
} from '../options/normalized-ecma-transform-plugin-config'

import { AliasPluginConfig } from '../options/normalized-alias-plugin-config'
import {
  BindingBuiltinPluginName,
  BindingGlobImportPluginConfig,
  BindingBuiltinPlugin,
  BindingManifestPluginConfig,
  BindingModulePreloadPolyfillPluginConfig,
  BindingJsonPluginConfig,
  BindingBuildImportAnalysisPluginConfig,
  BindingReplacePluginConfig,
  type BindingViteResolvePluginConfig,
  BindingCallableBuiltinPlugin,
  isCallableCompatibleBuiltinPlugin as isCallableCompatibleBuiltinPluginInternal,
} from '../binding'

export class BuiltinPlugin {
  constructor(
    public name: BindingBuiltinPluginName,
    public options?: unknown,
  ) {
    this.name = name
    this.options = options
  }
}

export class ModulePreloadPolyfillPlugin extends BuiltinPlugin {
  constructor(config?: BindingModulePreloadPolyfillPluginConfig) {
    super(BindingBuiltinPluginName.ModulePreloadPolyfillPlugin, config)
  }
}

export class DynamicImportVarsPlugin extends BuiltinPlugin {
  constructor() {
    super(BindingBuiltinPluginName.DynamicImportVarsPlugin)
  }
}

export class ImportGlobPlugin extends BuiltinPlugin {
  constructor(config?: BindingGlobImportPluginConfig) {
    super(BindingBuiltinPluginName.ImportGlobPlugin, config)
  }
}

export class ManifestPlugin extends BuiltinPlugin {
  constructor(config?: BindingManifestPluginConfig) {
    super(BindingBuiltinPluginName.ManifestPlugin, config)
  }
}

export class WasmHelperPlugin extends BuiltinPlugin {
  constructor() {
    super(BindingBuiltinPluginName.WasmHelperPlugin)
  }
}

export class WasmFallbackPlugin extends BuiltinPlugin {
  constructor() {
    super(BindingBuiltinPluginName.WasmFallbackPlugin)
  }
}

export class LoadFallbackPlugin extends BuiltinPlugin {
  constructor() {
    super(BindingBuiltinPluginName.LoadFallbackPlugin)
  }
}

export class AliasPlugin extends BuiltinPlugin {
  constructor(config?: AliasPluginConfig) {
    super(BindingBuiltinPluginName.AliasPlugin, config)
  }
}

export class TransformPlugin extends BuiltinPlugin {
  constructor(config?: TransformPluginConfig) {
    let normalizedConfig = normalizeEcmaTransformPluginConfig(config)
    super(BindingBuiltinPluginName.TransformPlugin, normalizedConfig)
  }
}

export class JsonPlugin extends BuiltinPlugin {
  constructor(config?: BindingJsonPluginConfig) {
    super(BindingBuiltinPluginName.JsonPlugin, config)
  }
}

export class BuildImportAnalysisPlugin extends BuiltinPlugin {
  constructor(config?: BindingBuildImportAnalysisPluginConfig) {
    super(BindingBuiltinPluginName.BuildImportAnalysisPlugin, config)
  }
}

export class ReplacePlugin extends BuiltinPlugin {
  constructor(config?: BindingReplacePluginConfig) {
    super(BindingBuiltinPluginName.ReplacePlugin, config)
  }
}

export class ViteResolvePlugin extends BuiltinPlugin {
  constructor(config?: BindingViteResolvePluginConfig) {
    super(BindingBuiltinPluginName.ViteResolvePlugin, config)
  }
}

export function modulePreloadPolyfillPlugin(
  config?: BindingModulePreloadPolyfillPluginConfig,
) {
  return new ModulePreloadPolyfillPlugin(config)
}

export function dynamicImportVarsPlugin() {
  return new DynamicImportVarsPlugin()
}

export function importGlobPlugin(config?: BindingGlobImportPluginConfig) {
  return new ImportGlobPlugin(config)
}

export function manifestPlugin(config?: BindingManifestPluginConfig) {
  return new ManifestPlugin(config)
}

export function wasmHelperPlugin() {
  return new WasmHelperPlugin()
}

export function wasmFallbackPlugin() {
  return new WasmFallbackPlugin()
}

export function transformPlugin(config?: TransformPluginConfig) {
  return new TransformPlugin(config)
}

export function loadFallbackPlugin() {
  return new LoadFallbackPlugin()
}

export function aliasPlugin(config: AliasPluginConfig) {
  return new AliasPlugin(config)
}

export function jsonPlugin(config?: BindingJsonPluginConfig) {
  return new JsonPlugin(config)
}

export function buildImportAnalysisPlugin(
  config: BindingBuildImportAnalysisPluginConfig,
) {
  return new BuildImportAnalysisPlugin(config)
}

export function viteResolvePlugin(
  config: Omit<BindingViteResolvePluginConfig, 'runtime'>,
) {
  return new ViteResolvePlugin({
    ...config,
    runtime: process.versions.deno
      ? 'deno'
      : process.versions.bun
        ? 'bun'
        : 'node',
  })
}

/**
 * ## Usage
 *
 * ```js
 * replacePlugin({
 *   'process.env.NODE_ENV': JSON.stringify('production'),
 *    __buildDate__: () => JSON.stringify(new Date()),
 *    __buildVersion: 15
 * })
 * ```
 *
 * ### With options
 *
 * ```js
 * replacePlugin({
 *   'process.env.NODE_ENV': JSON.stringify('production'),
 *   __buildDate__: () => JSON.stringify(new Date()),
 *   __buildVersion: 15
 * }, {
 *   preventAssignment: false,
 * })
 * ```
 *
 */
export function replacePlugin(
  values: BindingReplacePluginConfig['values'] = {},
  options: Omit<BindingReplacePluginConfig, 'values'> = {},
) {
  return new ReplacePlugin({ ...options, values })
}

export function isCallableCompatibleBuiltinPlugin(
  plugin: any,
): plugin is BuiltinPlugin {
  return (
    plugin instanceof BuiltinPlugin &&
    isCallableCompatibleBuiltinPluginInternal(bindingifyBuiltInPlugin(plugin))
  )
}

type BindingCallableBuiltinPluginLike = {
  [K in keyof BindingCallableBuiltinPlugin]: BindingCallableBuiltinPlugin[K]
}

export function makeBuiltinPluginCallable(plugin: BuiltinPlugin) {
  let callablePlugin = new BindingCallableBuiltinPlugin(
    bindingifyBuiltInPlugin(plugin),
  )

  const wrappedPlugin: Partial<BindingCallableBuiltinPluginLike> & {
    _original: BindingCallableBuiltinPlugin
  } = {
    _original: callablePlugin,
  }
  for (const [key, value] of Object.entries(callablePlugin)) {
    if (key === 'name') {
      wrappedPlugin[key] = value
    } else {
      // @ts-expect-error
      wrappedPlugin[key] = function (...args) {
        return value(...args)
      }
    }
  }
  return wrappedPlugin as BindingCallableBuiltinPluginLike & {
    _original: BindingCallableBuiltinPlugin
  }
}

export function isCallableBuiltinPlugin(plugin: any): boolean {
  return (
    '_original' in plugin &&
    plugin._original instanceof BindingCallableBuiltinPlugin
  )
}

export function bindingifyBuiltInPlugin(
  plugin: BuiltinPlugin,
): BindingBuiltinPlugin {
  return {
    __name: plugin.name,
    options: plugin.options,
  }
}
