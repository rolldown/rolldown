export { dev } from './api/dev';
export { DevEngine } from './api/dev/dev-engine';
export type { DevOptions, DevWatchOptions } from './api/dev/dev-options';
export { freeExternalMemory, scan } from './api/experimental';
export {
  type BindingClientHmrUpdate,
  BindingRebuildStrategy,
  createTokioRuntime,
  isolatedDeclaration,
  type IsolatedDeclarationsOptions,
  type IsolatedDeclarationsResult,
  isolatedDeclarationSync,
  minify,
  type MinifyOptions,
  type MinifyResult,
  minifySync,
  moduleRunnerTransform,
  type NapiResolveOptions as ResolveOptions,
  type ParseResult,
  type ParserOptions,
  type ResolveResult,
  ResolverFactory,
  transform,
  type TransformOptions,
  type TransformResult,
  transformSync,
} from './binding.cjs';
export { defineParallelPlugin } from './plugin/parallel-plugin';
export { parse, parseSync } from './utils/parse';
// Builtin plugin factory
export {
  isolatedDeclarationPlugin,
  viteAssetImportMetaUrlPlugin,
  viteBuildImportAnalysisPlugin,
  viteDynamicImportVarsPlugin,
  viteHtmlInlineProxyPlugin,
  viteImportGlobPlugin,
  viteJsonPlugin,
  viteLoadFallbackPlugin,
  viteModulePreloadPolyfillPlugin,
  viteReactRefreshWrapperPlugin,
  viteReporterPlugin,
  viteResolvePlugin,
  viteWasmFallbackPlugin,
  viteWasmHelperPlugin,
  viteWebWorkerPostPlugin,
} from './builtin-plugin/constructors';

export {
  /**
   * Alias of `viteDynamicImportVarsPlugin`. Note that this plugin is only intended to be used by Vite.
   */
  viteDynamicImportVarsPlugin as dynamicImportVarsPlugin,
  /**
   * Alias of `viteImportGlobPlugin`. Note that this plugin is only intended to be used by Vite.
   */
  viteImportGlobPlugin as importGlobPlugin,
} from './builtin-plugin/constructors';

export { viteAliasPlugin } from './builtin-plugin/alias-plugin';
export { viteAssetPlugin } from './builtin-plugin/asset-plugin';
export { viteTransformPlugin } from './builtin-plugin/transform-plugin';
export { viteCSSPlugin } from './builtin-plugin/vite-css-plugin';
export { viteCSSPostPlugin } from './builtin-plugin/vite-css-post-plugin';
export {
  viteHtmlPlugin,
  type ViteHtmlPluginOptions,
} from './builtin-plugin/vite-html-plugin';
export { viteManifestPlugin } from './builtin-plugin/vite-manifest-plugin';
