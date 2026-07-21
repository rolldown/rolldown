// CJS entry: `require()` of the ESM `./lib.js` forces CJS wrapping (`__commonJSMin`). The
// dependency's namespace object renders at chunk root as `var lib_exports = ...` and is read
// inside this closure as `(init_lib(), __toCommonJS(lib_exports))`.
//
// The author-local below is deliberately named `lib_exports` -- exactly the generated
// namespace-object name. Pre-fix it shadowed the captured chunk-root `lib_exports` inside the
// closure, so the require initializer became the self-referential
// `var lib_exports = (init_lib(), __toCommonJS(lib_exports))` -> `__toCommonJS(undefined)` ->
// `TypeError: Cannot convert undefined or null to object` (issue #9882, require()/namespace
// channel). After the fix the local is deconflicted (e.g. `lib_exports$1`).
var lib_exports = require('./lib.js');

module.exports = lib_exports.default() + ':' + lib_exports.named;
