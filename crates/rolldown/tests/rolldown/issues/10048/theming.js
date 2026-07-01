// Like storybook/dist/theming/index.js. It imports the interop helpers from
// helpers.js and CALLS them at the top level to wrap a CommonJS chain, AND it
// re-exports `themes` (from data.js), which does NOT depend on the helpers.
//
// Under viteMode this whole module body is kept as one lazy init once the module
// is reached, so the __commonJS / __toESM calls below survive. But because the
// only export the entry uses (`themes`) is a helper-independent passthrough,
// export-level tree-shaking drops helpers.js -> the calls below have no binding.
import { __commonJS, __toESM } from './helpers.js';
import { themes } from './data.js';

var require_react_is = __commonJS({ 'react-is.js'(exports) {
  exports.isElement = function (x) { return x != null && x.$$typeof === 1; };
} });

var require_hoist_non_react_statics = __commonJS({ 'hoist-non-react-statics.js'(exports, module) {
  var reactIs = require_react_is();
  module.exports = function hoist(target) { return reactIs.isElement(target) ? target : target; };
} });

var import_hoist_non_react_statics = __toESM(require_hoist_non_react_statics());

// A helper-dependent export the entry never imports; it only makes the helper
// calls above look "used" inside the module body (as emotion's `styled` does).
export function styled(Component) {
  return import_hoist_non_react_statics.default(Component);
}

export { themes };
