import type {
  BindingAssetPluginConfig,
  BindingBuildImportAnalysisPluginConfig,
  BindingDynamicImportVarsPluginConfig,
  BindingEsmExternalRequirePluginConfig,
  BindingImportGlobPluginConfig,
  BindingIsolatedDeclarationPluginConfig,
  BindingJsonPluginConfig,
  BindingManifestPluginConfig,
  BindingModulePreloadPolyfillPluginConfig,
  BindingOxcRuntimePluginConfig,
  BindingReporterPluginConfig,
  BindingViteResolvePluginConfig,
  BindingWasmHelperPluginConfig,
} from '../binding';
import type { StringOrRegExp } from '../types/utils';
import { normalizedStringOrRegex } from '../utils/normalize-string-or-regex';
import { BuiltinPlugin, makeBuiltinPluginCallable } from './utils';

export function modulePreloadPolyfillPlugin(
  config?: BindingModulePreloadPolyfillPluginConfig,
): BuiltinPlugin {
  return BuiltinPlugin.getInstance('builtin:module-preload-polyfill', config);
}

type DynamicImportVarsPluginConfig =
  & Omit<
    BindingDynamicImportVarsPluginConfig,
    'include' | 'exclude'
  >
  & {
    include?: StringOrRegExp | StringOrRegExp[];
    exclude?: StringOrRegExp | StringOrRegExp[];
  };

export function dynamicImportVarsPlugin(
  config?: DynamicImportVarsPluginConfig,
): BuiltinPlugin {
  if (config) {
    config.include = normalizedStringOrRegex(config.include);
    config.exclude = normalizedStringOrRegex(config.exclude);
  }
  return BuiltinPlugin.getInstance('builtin:dynamic-import-vars', config);
}

export function importGlobPlugin(
  config?: BindingImportGlobPluginConfig,
): BuiltinPlugin {
  return BuiltinPlugin.getInstance('builtin:import-glob', config);
}

export function reporterPlugin(
  config?: BindingReporterPluginConfig,
): BuiltinPlugin {
  return BuiltinPlugin.getInstance('builtin:reporter', config);
}

export function manifestPlugin(
  config?: BindingManifestPluginConfig,
): BuiltinPlugin {
  return BuiltinPlugin.getInstance('builtin:manifest', config);
}

export function wasmHelperPlugin(
  config?: BindingWasmHelperPluginConfig,
): BuiltinPlugin {
  return BuiltinPlugin.getInstance('builtin:wasm-helper', config);
}

export function wasmFallbackPlugin(): BuiltinPlugin {
  const builtinPlugin = BuiltinPlugin.getInstance('builtin:wasm-fallback');
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function loadFallbackPlugin(): BuiltinPlugin {
  return BuiltinPlugin.getInstance('builtin:load-fallback');
}

export function jsonPlugin(config?: BindingJsonPluginConfig): BuiltinPlugin {
  const builtinPlugin = BuiltinPlugin.getInstance('builtin:json', config);
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function buildImportAnalysisPlugin(
  config: BindingBuildImportAnalysisPluginConfig,
): BuiltinPlugin {
  return BuiltinPlugin.getInstance('builtin:build-import-analysis', config);
}

export function viteResolvePlugin(
  config: BindingViteResolvePluginConfig,
): BuiltinPlugin {
  const builtinPlugin = BuiltinPlugin.getInstance(
    'builtin:vite-resolve',
    config,
  );
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function isolatedDeclarationPlugin(
  config?: BindingIsolatedDeclarationPluginConfig,
): BuiltinPlugin {
  return BuiltinPlugin.getInstance('builtin:isolated-declaration', config);
}

export function assetPlugin(
  config?: BindingAssetPluginConfig,
): BuiltinPlugin {
  return BuiltinPlugin.getInstance('builtin:asset', config);
}

export function webWorkerPostPlugin(): BuiltinPlugin {
  return BuiltinPlugin.getInstance('builtin:web-worker-post');
}

export function oxcRuntimePlugin(
  config?: BindingOxcRuntimePluginConfig,
): BuiltinPlugin {
  return BuiltinPlugin.getInstance('builtin:oxc-runtime', config);
}

export function esmExternalRequirePlugin(
  config?: BindingEsmExternalRequirePluginConfig,
): BuiltinPlugin {
  return BuiltinPlugin.getInstance('builtin:esm-external-require', config);
}
