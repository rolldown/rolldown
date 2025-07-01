export { experimental_scan as scan } from './api/experimental';
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
  importGlobPlugin,
  isolatedDeclarationPlugin,
  jsonPlugin,
  loadFallbackPlugin,
  manifestPlugin,
  moduleFederationPlugin,
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
