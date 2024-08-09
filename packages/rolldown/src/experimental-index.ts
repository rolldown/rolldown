import { defineParallelPlugin } from './plugin/parallel-plugin'
import { experimental_scan } from './rolldown'
import {
  modulePreloadPolyfillPlugin,
  dynamicImportVarsPlugin,
  globImportPlugin,
  manifestPlugin,
  wasmPlugin,
  ecmaTransformPlugin
} from './plugin/builtin-plugin'
import { transform } from './binding'

export { defineParallelPlugin, experimental_scan as scan, transform }

// Builtin plugin factory
export {
  modulePreloadPolyfillPlugin,
  dynamicImportVarsPlugin,
  wasmPlugin,
  globImportPlugin,
  manifestPlugin,
  ecmaTransformPlugin
}
