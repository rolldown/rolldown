import {
  BindingBuiltinPluginName,
  BindingGlobImportPluginConfig,
  BindingManifestPluginConfig,
  BindingModulePreloadPolyfillPluginConfig,
  BindingJsonPluginConfig,
  BindingBuildImportAnalysisPluginConfig,
  type BindingViteResolvePluginConfig,
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
    super(BindingBuiltinPluginName.ModulePreloadPolyfill, config)
  }
}

export class DynamicImportVarsPlugin extends BuiltinPlugin {
  constructor() {
    super(BindingBuiltinPluginName.DynamicImportVars)
  }
}

export class ImportGlobPlugin extends BuiltinPlugin {
  constructor(config?: BindingGlobImportPluginConfig) {
    super(BindingBuiltinPluginName.ImportGlob, config)
  }
}

export class ManifestPlugin extends BuiltinPlugin {
  constructor(config?: BindingManifestPluginConfig) {
    super(BindingBuiltinPluginName.Manifest, config)
  }
}

export class WasmHelperPlugin extends BuiltinPlugin {
  constructor() {
    super(BindingBuiltinPluginName.WasmHelper)
  }
}

export class WasmFallbackPlugin extends BuiltinPlugin {
  constructor() {
    super(BindingBuiltinPluginName.WasmFallback)
  }
}

export class LoadFallbackPlugin extends BuiltinPlugin {
  constructor() {
    super(BindingBuiltinPluginName.LoadFallback)
  }
}

export class JsonPlugin extends BuiltinPlugin {
  constructor(config?: BindingJsonPluginConfig) {
    super(BindingBuiltinPluginName.Json, config)
  }
}

export class BuildImportAnalysisPlugin extends BuiltinPlugin {
  constructor(config?: BindingBuildImportAnalysisPluginConfig) {
    super(BindingBuiltinPluginName.BuildImportAnalysis, config)
  }
}

export class ViteResolvePlugin extends BuiltinPlugin {
  constructor(config?: BindingViteResolvePluginConfig) {
    super(BindingBuiltinPluginName.ViteResolve, config)
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

export function loadFallbackPlugin() {
  return new LoadFallbackPlugin()
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
