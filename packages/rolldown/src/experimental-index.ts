export { dev } from './api/dev';
export { DevEngine } from './api/dev/dev-engine';
export type { DevOptions, DevWatchOptions } from './api/dev/dev-options';
export { scan } from './api/experimental';
export {
  BindingClientHmrUpdate,
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
export { defineParallelPlugin } from './plugin/parallel-plugin';
// Builtin plugin factory
export {
  aliasPlugin,
  assetPlugin,
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
  replacePlugin,
  reporterPlugin,
  transformPlugin,
  viteResolvePlugin,
  wasmFallbackPlugin,
  wasmHelperPlugin,
  webWorkerPostPlugin,
} from './builtin-plugin';
