export { dev } from './api/dev';
export { DevEngine } from './api/dev/dev-engine';
export type { DevOptions, DevWatchOptions } from './api/dev/dev-options';
export { scan } from './api/experimental';
export {
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
  reporterPlugin,
  viteResolvePlugin,
  wasmFallbackPlugin,
  wasmHelperPlugin,
  webWorkerPostPlugin,
} from './builtin-plugin/constructors';

export { aliasPlugin } from './builtin-plugin/alias-plugin';
export { replacePlugin } from './builtin-plugin/replace-plugin';
export { transformPlugin } from './builtin-plugin/transform-plugin';
