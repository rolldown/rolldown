<<<<<<< HEAD
export { defineParallelPlugin } from './plugin/parallel-plugin'
export { experimental_scan as scan } from './rolldown'
export { transform } from './binding'
export { composeJsPlugins as composePlugins } from './utils/compose-js-plugins'
||||||| parent of 59893bfc (fix: 🐛 should stringify twice)
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

=======
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
  jsonPlugin
} from './plugin/builtin-plugin'
import { transform } from './binding'

export { defineParallelPlugin, experimental_scan as scan, transform }

>>>>>>> 59893bfc (fix: 🐛 should stringify twice)
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
  jsonPlugin
} from './plugin/builtin-plugin'
}
