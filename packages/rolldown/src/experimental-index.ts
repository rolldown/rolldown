export { defineParallelPlugin } from './plugin/parallel-plugin'
export { experimental_scan as scan } from './rolldown'
export { transform } from './binding'
export { composeJsPlugins as composePlugins } from './utils/compose-js-plugins'
import { defineParallelPlugin } from './plugin/parallel-plugin'
import { experimental_scan } from './rolldown'
import {
  modulePreloadPolyfillPlugin,
  dynamicImportVarsPlugin,
  globImportPlugin,
  manifestPlugin,
  wasmPlugin,
  loadFallbackPlugin,
  transformPlugin,
} from './plugin/builtin-plugin'
import { transform } from './binding'

export { defineParallelPlugin, experimental_scan as scan, transform }

