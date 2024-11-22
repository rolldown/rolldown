export { defineParallelPlugin } from './plugin/parallel-plugin'
export { experimental_scan as scan } from './rolldown'
export { transform } from './binding'
export type { TransformOptions, TransformResult } from './binding'
export { composeJsPlugins as composePlugins } from './utils/compose-js-plugins'
// Builtin plugin factory
export {
  modulePreloadPolyfillPlugin,
  dynamicImportVarsPlugin,
  wasmHelperPlugin,
  wasmFallbackPlugin,
  importGlobPlugin,
  manifestPlugin,
  loadFallbackPlugin,
  jsonPlugin,
  buildImportAnalysisPlugin,
  viteResolvePlugin,
} from './builtin-plugin/constructors'

export { transformPlugin } from './builtin-plugin/transform-plugin'
export { replacePlugin } from './builtin-plugin/replace-plugin'
export { aliasPlugin } from './builtin-plugin/alias-plugin'
export {
  isCallableCompatibleBuiltinPlugin,
  makeBuiltinPluginCallable,
  isCallableBuiltinPlugin,
} from './builtin-plugin/utils'
