export { defineParallelPlugin } from './plugin/parallel-plugin'
export { experimental_scan as scan } from './rolldown'
export { transform } from './binding'

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
} from './plugin/builtin-plugin'
