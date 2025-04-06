export { experimental_scan as scan } from './api/experimental';
export { moduleRunnerTransform, transform } from './binding';
export type { TransformOptions, TransformResult } from './binding';
export { defineParallelPlugin } from './plugin/parallel-plugin';
export { composeJsPlugins as composePlugins } from './utils/compose-js-plugins';
// Builtin plugin factory
export {
  buildImportAnalysisPlugin,
  dynamicImportVarsPlugin,
  importGlobPlugin,
  isolatedDeclarationPlugin,
  jsonPlugin,
  loadFallbackPlugin,
  manifestPlugin,
  moduleFederationPlugin,
  modulePreloadPolyfillPlugin,
  viteResolvePlugin,
  wasmFallbackPlugin,
  wasmHelperPlugin,
} from './builtin-plugin/constructors';

export { aliasPlugin } from './builtin-plugin/alias-plugin';
export { replacePlugin } from './builtin-plugin/replace-plugin';
export { transformPlugin } from './builtin-plugin/transform-plugin';
