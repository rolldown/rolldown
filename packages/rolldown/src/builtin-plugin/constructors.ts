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

export function modulePreloadPolyfillPlugin(
  config?: BindingModulePreloadPolyfillPluginConfig,
) {
  return new BuiltinPlugin('builtin:module-preload-polyfill', config)
}

export function dynamicImportVarsPlugin() {
  return new BuiltinPlugin('builtin:dynamic-import-vars')
}

export function importGlobPlugin(config?: BindingGlobImportPluginConfig) {
  return new BuiltinPlugin('builtin:import-glob', config)
}

export function manifestPlugin(config?: BindingManifestPluginConfig) {
  return new BuiltinPlugin('builtin:manifest', config)
}

export function wasmHelperPlugin() {
  return new BuiltinPlugin('builtin:wasm-helper')
}

export function wasmFallbackPlugin() {
  return new BuiltinPlugin('builtin:wasm-fallback')
}

export function loadFallbackPlugin() {
  return new BuiltinPlugin('builtin:load-fallback')
}

export function jsonPlugin(config?: BindingJsonPluginConfig) {
  return new BuiltinPlugin('builtin:json', config)
}

export function buildImportAnalysisPlugin(
  config: BindingBuildImportAnalysisPluginConfig,
) {
  return new BuiltinPlugin('builtin:build-import-analysis', config)
}

export function viteResolvePlugin(
  config: Omit<BindingViteResolvePluginConfig, 'runtime'>,
) {
  return new BuiltinPlugin('builtin:vite-resolve', {
    ...config,
    runtime: process.versions.deno
      ? 'deno'
      : process.versions.bun
        ? 'bun'
        : 'node',
  })
}
