import {
  type BindingBuiltinPluginName,
  BindingGlobImportPluginConfig,
  BindingManifestPluginConfig,
  BindingModulePreloadPolyfillPluginConfig,
  BindingJsonPluginConfig,
  BindingBuildImportAnalysisPluginConfig,
  type BindingViteResolvePluginConfig,
  BindingModuleFederationPluginOption,
} from '../binding'
import { makeBuiltinPluginCallable } from './utils'

export class BuiltinPlugin {
  constructor(
    public name: BindingBuiltinPluginName,
    // NOTE: has `_` to avoid conflict with `options` hook
    public _options?: unknown,
  ) {
    this.name = name
    this._options = _options
  }
}

export function modulePreloadPolyfillPlugin(
  config?: BindingModulePreloadPolyfillPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:module-preload-polyfill', config)
}

export function dynamicImportVarsPlugin(): BuiltinPlugin {
  return new BuiltinPlugin('builtin:dynamic-import-vars')
}

export function importGlobPlugin(
  config?: BindingGlobImportPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:import-glob', config)
}

export function manifestPlugin(
  config?: BindingManifestPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:manifest', config)
}

export function wasmHelperPlugin(): BuiltinPlugin {
  return new BuiltinPlugin('builtin:wasm-helper')
}

export function wasmFallbackPlugin(): BuiltinPlugin {
  return new BuiltinPlugin('builtin:wasm-fallback')
}

export function loadFallbackPlugin(): BuiltinPlugin {
  return new BuiltinPlugin('builtin:load-fallback')
}

export function jsonPlugin(config?: BindingJsonPluginConfig): BuiltinPlugin {
  return new BuiltinPlugin('builtin:json', config)
}

export function buildImportAnalysisPlugin(
  config: BindingBuildImportAnalysisPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:build-import-analysis', config)
}

export function viteResolvePlugin(
  config: Omit<BindingViteResolvePluginConfig, 'runtime'>,
): BuiltinPlugin {
  const builtinPlugin = new BuiltinPlugin('builtin:vite-resolve', {
    ...config,
    runtime: process.versions.deno
      ? 'deno'
      : process.versions.bun
        ? 'bun'
        : 'node',
  })
  return makeBuiltinPluginCallable(builtinPlugin)
}

export function moduleFederationPlugin(
  config: BindingModuleFederationPluginOption,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:load-fallback', config)
}
