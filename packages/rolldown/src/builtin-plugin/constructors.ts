import {
  type BindingBuiltinPluginName,
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
    super('builtin:module-preload-polyfill', config)
  }
}

export class DynamicImportVarsPlugin extends BuiltinPlugin {
  constructor() {
    super('builtin:dynamic-import-vars')
  }
}

export class ImportGlobPlugin extends BuiltinPlugin {
  constructor(config?: BindingGlobImportPluginConfig) {
    super('builtin:import-glob', config)
  }
}

export class ManifestPlugin extends BuiltinPlugin {
  constructor(config?: BindingManifestPluginConfig) {
    super('builtin:manifest', config)
  }
}

export class WasmHelperPlugin extends BuiltinPlugin {
  constructor() {
    super('builtin:wasm-helper')
  }
}

export class WasmFallbackPlugin extends BuiltinPlugin {
  constructor() {
    super('builtin:wasm-fallback')
  }
}

export class LoadFallbackPlugin extends BuiltinPlugin {
  constructor() {
    super('builtin:load-fallback')
  }
}

export class JsonPlugin extends BuiltinPlugin {
  constructor(config?: BindingJsonPluginConfig) {
    super('builtin:json', config)
  }
}

export class BuildImportAnalysisPlugin extends BuiltinPlugin {
  constructor(config?: BindingBuildImportAnalysisPluginConfig) {
    super('builtin:build-import-analysis', config)
  }
}

export class ViteResolvePlugin extends BuiltinPlugin {
  constructor(config?: BindingViteResolvePluginConfig) {
    super('builtin:vite-resolve', config)
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
