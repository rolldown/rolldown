export { defineParallelPlugin } from './plugin/parallel-plugin'
export { experimental_scan as scan } from './rolldown'
export { transform } from './binding'
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
  transformPlugin,
  aliasPlugin,
  jsonPlugin,
  buildImportAnalysisPlugin,
  replacePlugin,
} from './plugin/builtin-plugin'
