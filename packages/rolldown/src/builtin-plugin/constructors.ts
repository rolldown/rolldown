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
import {
  BuiltinPlugin,
  createBuiltinPlugin,
  makeBuiltinPluginCallable,
} from './utils';

export function modulePreloadPolyfillPlugin(
  config?: BindingModulePreloadPolyfillPluginConfig,
): BuiltinPlugin {
  return createBuiltinPlugin('builtin:module-preload-polyfill', config);
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
  return createBuiltinPlugin('builtin:dynamic-import-vars', config);
}

export function importGlobPlugin(
  config?: BindingImportGlobPluginConfig,
): BuiltinPlugin {
  return createBuiltinPlugin('builtin:import-glob', config);
}

export function reporterPlugin(
  config?: BindingReporterPluginConfig,
): BuiltinPlugin {
  return createBuiltinPlugin('builtin:reporter', config);
}

export function manifestPlugin(
  config?: BindingManifestPluginConfig,
): BuiltinPlugin {
  return createBuiltinPlugin('builtin:manifest', config);
}

export function wasmHelperPlugin(
  config?: BindingWasmHelperPluginConfig,
): BuiltinPlugin {
  return createBuiltinPlugin('builtin:wasm-helper', config);
}

export function wasmFallbackPlugin(): BuiltinPlugin {
  const builtinPlugin = createBuiltinPlugin('builtin:wasm-fallback');
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function loadFallbackPlugin(): BuiltinPlugin {
  return createBuiltinPlugin('builtin:load-fallback');
}

export function jsonPlugin(config?: BindingJsonPluginConfig): BuiltinPlugin {
  const builtinPlugin = createBuiltinPlugin('builtin:json', config);
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function buildImportAnalysisPlugin(
  config: BindingBuildImportAnalysisPluginConfig,
): BuiltinPlugin {
  return createBuiltinPlugin('builtin:build-import-analysis', config);
}

export function viteResolvePlugin(
  config: BindingViteResolvePluginConfig,
): BuiltinPlugin {
  const builtinPlugin = createBuiltinPlugin('builtin:vite-resolve', config);
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function isolatedDeclarationPlugin(
  config?: BindingIsolatedDeclarationPluginConfig,
): BuiltinPlugin {
  return createBuiltinPlugin('builtin:isolated-declaration', config);
}

export function assetPlugin(
  config?: BindingAssetPluginConfig,
): BuiltinPlugin {
  return createBuiltinPlugin('builtin:asset', config);
}

export function webWorkerPostPlugin(): BuiltinPlugin {
  return createBuiltinPlugin('builtin:web-worker-post');
}

export function oxcRuntimePlugin(
  config?: BindingOxcRuntimePluginConfig,
): BuiltinPlugin {
  return createBuiltinPlugin('builtin:oxc-runtime', config);
}

export function esmExternalRequirePlugin(
  config?: BindingEsmExternalRequirePluginConfig,
): BuiltinPlugin {
  return createBuiltinPlugin('builtin:esm-external-require', config);
}
