import {
  type BindingBuiltinPluginName,
  BindingGlobImportPluginConfig,
  BindingManifestPluginConfig,
  BindingModulePreloadPolyfillPluginConfig,
  BindingJsonPluginConfig,
  BindingBuildImportAnalysisPluginConfig,
  type BindingViteResolvePluginConfig,
  BindingModuleFederationPluginOption,
  BindingRemote,
  BindingMfManifest,
  BindingIsolatedDeclarationPluginConfig,
} from '../binding'
import { makeBuiltinPluginCallable } from './utils'

export class BuiltinPlugin {
  constructor(
    public name: BindingBuiltinPluginName,
    // NOTE: has `_` to avoid conflict with `options` hook
    public _options?: unknown,
  ) {}
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

export type ModuleFederationPluginOption = Omit<
  BindingModuleFederationPluginOption,
  'remotes'
> & {
  remotes?: Record<string, string | BindingRemote>
  manifest?: boolean | BindingMfManifest
}

export function moduleFederationPlugin(
  config: ModuleFederationPluginOption,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:module-federation', {
    ...config,
    remotes:
      config.remotes &&
      Object.entries(config.remotes).map(([name, remote]) => {
        if (typeof remote === 'string') {
          const [entryGlobalName] = remote.split('@')
          const entry = remote.replace(entryGlobalName + '@', '')
          return { entry, name, entryGlobalName }
        }
        return {
          ...remote,
          name: remote.name ?? name,
        }
      }),
    manifest:
      config.manifest === false
        ? undefined
        : config.manifest === true
          ? {}
          : config.manifest,
  })
}

export function isolatedDeclarationPlugin(
  config?: BindingIsolatedDeclarationPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:isolated-declaration', config)
}
