import {
  normalizeEcmaTransformPluginConfig,
  TransformPluginConfig,
} from '../options/normalized-ecma-transform-plugin-config'

import {
  AliasPluginConfig,
  normalizeAliasPluginConfig,
} from '../options/normalized-alias-plugin-config'
import {
  BindingBuiltinPluginName,
  BindingGlobImportPluginConfig,
  BindingBuiltinPlugin,
  BindingManifestPluginConfig,
  BindingModulePreloadPolyfillPluginConfig,
  BindingJsonPluginConfig,
  BindingBuildImportAnalysisPluginConfig,
  BindingReplacePluginConfig,
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
    let normalizedAliasPluginConfig = normalizeAliasPluginConfig(config)
    super(BindingBuiltinPluginName.AliasPlugin, normalizedAliasPluginConfig)
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

export function bindingifyBuiltInPlugin(
  plugin: BuiltinPlugin,
): BindingBuiltinPlugin {
  return {
    __name: plugin.name,
    options: plugin.options,
  }
}
