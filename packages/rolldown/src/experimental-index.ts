import { defineParallelPlugin } from './plugin/parallel-plugin'
import { experimental_scan } from './rolldown'
import {
  dynamicImportVarsPlugin,
  globImportPlugin,
  wasmPlugin,
} from './plugin/builtin-plugin'
import { transform } from './binding'

export { defineParallelPlugin, experimental_scan as scan, transform }

// Builtin plugin factory
export { dynamicImportVarsPlugin, wasmPlugin, globImportPlugin }
