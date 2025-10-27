export { dev } from './api/dev';
export { DevEngine } from './api/dev/dev-engine';
export type { DevOptions, DevWatchOptions } from './api/dev/dev-options';
export { freeExternalMemory, scan } from './api/experimental';
export {
  BindingRebuildStrategy,
  isolatedDeclaration,
  type IsolatedDeclarationsOptions,
  type IsolatedDeclarationsResult,
  moduleRunnerTransform,
  type NapiResolveOptions as ResolveOptions,
  type ResolveResult,
  ResolverFactory,
  transform,
  type TransformOptions,
  type TransformResult,
} from './binding';
export type { BindingClientHmrUpdate } from './binding';
export { defineParallelPlugin } from './plugin/parallel-plugin';
// Builtin plugin factory
export {
  buildImportAnalysisPlugin,
  dynamicImportVarsPlugin,
  esmExternalRequirePlugin,
  importGlobPlugin,
  isolatedDeclarationPlugin,
  jsonPlugin,
  loadFallbackPlugin,
  manifestPlugin,
  modulePreloadPolyfillPlugin,
  reactRefreshWrapperPlugin,
  reporterPlugin,
  viteCSSPostPlugin,
  viteHtmlPlugin,
  viteResolvePlugin,
  wasmFallbackPlugin,
  wasmHelperPlugin,
  webWorkerPostPlugin,
} from './builtin-plugin/constructors';

export { aliasPlugin } from './builtin-plugin/alias-plugin';
export { assetPlugin } from './builtin-plugin/asset-plugin';
export { replacePlugin } from './builtin-plugin/replace-plugin';
export { transformPlugin } from './builtin-plugin/transform-plugin';
export { viteCSSPlugin } from './builtin-plugin/vite-css-plugin';
