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

export class GlobImportPlugin extends BuiltinPlugin {
  constructor(config?: BindingGlobImportPluginConfig) {
    super(BindingBuiltinPluginName.GlobImportPlugin, config)
  }
}

export class ManifestPlugin extends BuiltinPlugin {
  constructor(config?: BindingManifestPluginConfig) {
    super(BindingBuiltinPluginName.ManifestPlugin, config)
  }
}

export class WasmPlugin extends BuiltinPlugin {
  constructor() {
    super(BindingBuiltinPluginName.WasmPlugin)
  }
}

export class EcmaTransformPlugin extends BuiltinPlugin {
  constructor(config?: TransformPluginConfig) {
    let normalizedConfig = normalizedEcmaTransformPluginConfig(config)
    super(BindingBuiltinPluginName.EcmaTransformPlugin, normalizedConfig)
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

export function globImportPlugin(config?: BindingGlobImportPluginConfig) {
  return new GlobImportPlugin(config)
}

export function manifestPlugin(config?: BindingManifestPluginConfig) {
  return new ManifestPlugin(config)
}

export function wasmPlugin() {
  return new WasmPlugin()
}

export function ecmaTransformPlugin(config?: TransformPluginConfig) {
  return new EcmaTransformPlugin(config)
}

export function bindingifyBuiltInPlugin(
  plugin: BuiltinPlugin,
): BindingBuiltinPlugin {
  return {
    __name: plugin.name,
    options: plugin.options,
  }
}
