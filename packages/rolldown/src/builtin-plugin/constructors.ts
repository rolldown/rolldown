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
  return new BuiltinPlugin('builtin:module-preload-polyfill', config);
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
  return new BuiltinPlugin('builtin:dynamic-import-vars', config);
}

export function importGlobPlugin(
  config?: BindingImportGlobPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:import-glob', config);
}

export function reporterPlugin(
  config?: BindingReporterPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:reporter', config);
}

export function manifestPlugin(
  config?: BindingManifestPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:manifest', config);
}

export function wasmHelperPlugin(
  config?: BindingWasmHelperPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:wasm-helper', config);
}

export function wasmFallbackPlugin(): BuiltinPlugin {
  const builtinPlugin = new BuiltinPlugin('builtin:wasm-fallback');
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function loadFallbackPlugin(): BuiltinPlugin {
  return new BuiltinPlugin('builtin:load-fallback');
}

export function jsonPlugin(config?: BindingJsonPluginConfig): BuiltinPlugin {
  const builtinPlugin = new BuiltinPlugin('builtin:json', config);
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function buildImportAnalysisPlugin(
  config: BindingBuildImportAnalysisPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:build-import-analysis', config);
}

export function viteResolvePlugin(
  config: BindingViteResolvePluginConfig,
): BuiltinPlugin {
  const builtinPlugin = new BuiltinPlugin('builtin:vite-resolve', config);
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function isolatedDeclarationPlugin(
  config?: BindingIsolatedDeclarationPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:isolated-declaration', config);
}

export function assetPlugin(
  config?: BindingAssetPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:asset', config);
}

export function webWorkerPostPlugin(): BuiltinPlugin {
  return new BuiltinPlugin('builtin:web-worker-post');
}

export function oxcRuntimePlugin(
  config?: BindingOxcRuntimePluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:oxc-runtime', config);
}

export function esmExternalRequirePlugin(
  config?: BindingEsmExternalRequirePluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:esm-external-require', config);
}
