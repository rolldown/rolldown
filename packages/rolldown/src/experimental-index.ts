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
  buildImportAnalysisPlugin,
  dynamicImportVarsPlugin,
  htmlInlineProxyPlugin,
  importGlobPlugin,
  isolatedDeclarationPlugin,
  jsonPlugin,
  loadFallbackPlugin,
  manifestPlugin,
  modulePreloadPolyfillPlugin,
  reactRefreshWrapperPlugin,
  reporterPlugin,
  viteCSSPostPlugin,
  viteResolvePlugin,
  wasmFallbackPlugin,
  wasmHelperPlugin,
  webWorkerPostPlugin,
} from './builtin-plugin/constructors';

export { aliasPlugin } from './builtin-plugin/alias-plugin';
export { viteAssetPlugin } from './builtin-plugin/asset-plugin';
export { transformPlugin } from './builtin-plugin/transform-plugin';
export { viteCSSPlugin } from './builtin-plugin/vite-css-plugin';
export {
  viteHtmlPlugin,
  type ViteHtmlPluginOptions,
} from './builtin-plugin/vite-html-plugin';
