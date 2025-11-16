export { dev } from './api/dev';
export { DevEngine } from './api/dev/dev-engine';
export type { DevOptions, DevWatchOptions } from './api/dev/dev-options';
export { freeExternalMemory, scan } from './api/experimental';
export {
  type BindingClientHmrUpdate,
  BindingRebuildStrategy,
  isolatedDeclaration,
  type IsolatedDeclarationsOptions,
  type IsolatedDeclarationsResult,
  minify,
  type MinifyOptions,
  type MinifyResult,
  moduleRunnerTransform,
  type NapiResolveOptions as ResolveOptions,
  parseAsync,
  type ParseResult,
  type ParserOptions,
  parseSync,
  type ResolveResult,
  ResolverFactory,
  transform,
  type TransformOptions,
  type TransformResult,
} from './binding.cjs';
export { defineParallelPlugin } from './plugin/parallel-plugin';
// Builtin plugin factory
export {
  isolatedDeclarationPlugin,
  modulePreloadPolyfillPlugin,
  reactRefreshWrapperPlugin,
  reporterPlugin,
  viteBuildImportAnalysisPlugin,
  viteCSSPostPlugin,
  viteDynamicImportVarsPlugin,
  viteHtmlInlineProxyPlugin,
  viteImportGlobPlugin,
  viteJsonPlugin,
  viteLoadFallbackPlugin,
  viteManifestPlugin,
  viteResolvePlugin,
  wasmFallbackPlugin,
  wasmHelperPlugin,
  webWorkerPostPlugin,
} from './builtin-plugin/constructors';

export { viteAliasPlugin } from './builtin-plugin/alias-plugin';
export { viteAssetPlugin } from './builtin-plugin/asset-plugin';
export { transformPlugin } from './builtin-plugin/transform-plugin';
export { viteCSSPlugin } from './builtin-plugin/vite-css-plugin';
export {
  viteHtmlPlugin,
  type ViteHtmlPluginOptions,
} from './builtin-plugin/vite-html-plugin';
