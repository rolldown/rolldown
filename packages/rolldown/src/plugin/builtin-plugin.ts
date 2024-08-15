import {
  normalizedEcmaTransformPluginConfig,
  TransformPluginConfig,
} from '../options/normalized-ecma-transform-plugin-config'
import {
  BindingBuiltinPluginName,
  BindingGlobImportPluginConfig,
  BindingBuiltinPlugin,
  BindingManifestPluginConfig,
  BindingModulePreloadPolyfillPluginConfig,
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

export class TransformPlugin extends BuiltinPlugin {
  constructor(config?: TransformPluginConfig) {
    let normalizedConfig = normalizedEcmaTransformPluginConfig(config)
    super(BindingBuiltinPluginName.TransformPlugin, normalizedConfig)
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
export function bindingifyBuiltInPlugin(
  plugin: BuiltinPlugin,
): BindingBuiltinPlugin {
  return {
    __name: plugin.name,
    options: plugin.options,
  }
}
