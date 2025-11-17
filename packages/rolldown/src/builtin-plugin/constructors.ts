import type {
  BindingEsmExternalRequirePluginConfig,
  BindingIsolatedDeclarationPluginConfig,
  BindingViteBuildImportAnalysisPluginConfig,
  BindingViteCssPostPluginConfig,
  BindingViteDynamicImportVarsPluginConfig,
  BindingViteHtmlInlineProxyPluginConfig,
  BindingViteImportGlobPluginConfig,
  BindingViteJsonPluginConfig,
  BindingViteManifestPluginConfig,
  BindingViteModulePreloadPolyfillPluginConfig,
  BindingViteReactRefreshWrapperPluginConfig,
  BindingViteReporterPluginConfig,
  BindingViteResolvePluginConfig,
  BindingViteWasmHelperPluginConfig,
} from '../binding.cjs';
import type { StringOrRegExp } from '../types/utils';
import { normalizedStringOrRegex } from '../utils/normalize-string-or-regex';
import { BuiltinPlugin, makeBuiltinPluginCallable } from './utils';

export function viteModulePreloadPolyfillPlugin(
  config?: BindingViteModulePreloadPolyfillPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-module-preload-polyfill', config);
}

type DynamicImportVarsPluginConfig =
  & Omit<
    BindingViteDynamicImportVarsPluginConfig,
    'include' | 'exclude'
  >
  & {
    include?: StringOrRegExp | StringOrRegExp[];
    exclude?: StringOrRegExp | StringOrRegExp[];
  };

export function viteDynamicImportVarsPlugin(
  config?: DynamicImportVarsPluginConfig,
): BuiltinPlugin {
  if (config) {
    config.include = normalizedStringOrRegex(config.include);
    config.exclude = normalizedStringOrRegex(config.exclude);
  }
  return new BuiltinPlugin('builtin:vite-dynamic-import-vars', config);
}

export function viteImportGlobPlugin(
  config?: BindingViteImportGlobPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-import-glob', config);
}

export function viteReporterPlugin(
  config?: BindingViteReporterPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-reporter', config);
}

export function viteManifestPlugin(
  config?: BindingViteManifestPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-manifest', config);
}

export function viteWasmHelperPlugin(
  config?: BindingViteWasmHelperPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-wasm-helper', config);
}

export function viteWasmFallbackPlugin(): BuiltinPlugin {
  const builtinPlugin = new BuiltinPlugin('builtin:vite-wasm-fallback');
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function viteLoadFallbackPlugin(): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-load-fallback');
}

export function viteJsonPlugin(
  config?: BindingViteJsonPluginConfig,
): BuiltinPlugin {
  const builtinPlugin = new BuiltinPlugin('builtin:vite-json', config);
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function viteBuildImportAnalysisPlugin(
  config: BindingViteBuildImportAnalysisPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-build-import-analysis', config);
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

export function viteWebWorkerPostPlugin(): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-web-worker-post');
}

export function esmExternalRequirePlugin(
  config?: BindingEsmExternalRequirePluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:esm-external-require', config);
}

type ViteReactRefreshWrapperPluginConfig =
  & Omit<
    BindingViteReactRefreshWrapperPluginConfig,
    'include' | 'exclude'
  >
  & {
    include?: StringOrRegExp | StringOrRegExp[];
    exclude?: StringOrRegExp | StringOrRegExp[];
  };

export function viteReactRefreshWrapperPlugin(
  config: ViteReactRefreshWrapperPluginConfig,
): BuiltinPlugin {
  if (config) {
    config.include = normalizedStringOrRegex(config.include);
    config.exclude = normalizedStringOrRegex(config.exclude);
  }
  const builtinPlugin = new BuiltinPlugin(
    'builtin:vite-react-refresh-wrapper',
    config,
  );
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function viteCSSPostPlugin(
  config?: BindingViteCssPostPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-css-post', config);
}

export function viteHtmlInlineProxyPlugin(
  config: BindingViteHtmlInlineProxyPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-html-inline-proxy', config);
}
