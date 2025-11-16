import type {
  BindingEsmExternalRequirePluginConfig,
  BindingIsolatedDeclarationPluginConfig,
  BindingModulePreloadPolyfillPluginConfig,
  BindingReactRefreshWrapperPluginConfig,
  BindingReporterPluginConfig,
  BindingViteBuildImportAnalysisPluginConfig,
  BindingViteCssPostPluginConfig,
  BindingViteDynamicImportVarsPluginConfig,
  BindingViteHtmlInlineProxyPluginConfig,
  BindingViteImportGlobPluginConfig,
  BindingViteJsonPluginConfig,
  BindingViteManifestPluginConfig,
  BindingViteResolvePluginConfig,
  BindingWasmHelperPluginConfig,
} from '../binding.cjs';
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

export function reporterPlugin(
  config?: BindingReporterPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:reporter', config);
}

export function viteManifestPlugin(
  config?: BindingViteManifestPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-manifest', config);
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

export function webWorkerPostPlugin(): BuiltinPlugin {
  return new BuiltinPlugin('builtin:web-worker-post');
}

export function esmExternalRequirePlugin(
  config?: BindingEsmExternalRequirePluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:esm-external-require', config);
}

type ReactRefreshWrapperPluginConfig =
  & Omit<
    BindingReactRefreshWrapperPluginConfig,
    'include' | 'exclude'
  >
  & {
    include?: StringOrRegExp | StringOrRegExp[];
    exclude?: StringOrRegExp | StringOrRegExp[];
  };

export function reactRefreshWrapperPlugin(
  config: ReactRefreshWrapperPluginConfig,
): BuiltinPlugin {
  if (config) {
    config.include = normalizedStringOrRegex(config.include);
    config.exclude = normalizedStringOrRegex(config.exclude);
  }
  const builtinPlugin = new BuiltinPlugin(
    'builtin:react-refresh-wrapper',
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
