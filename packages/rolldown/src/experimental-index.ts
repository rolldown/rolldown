export { defineParallelPlugin } from './plugin/parallel-plugin'
export { experimental_scan as scan } from './rolldown'
export { transform } from './binding'

// Builtin plugin factory
export {
  modulePreloadPolyfillPlugin,
  dynamicImportVarsPlugin,
  wasmPlugin,
  importGlobPlugin,
  manifestPlugin,
  loadFallbackPlugin,
  transformPlugin,
} from './plugin/builtin-plugin'
