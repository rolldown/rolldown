// `require('./esm.js')` of an ES module prints as `(init_esm(), __toCommonJS(esm_exports))`, so this
// local, named like the runtime helper as in rolldown's own CJS output, must not capture it.
var __toCommonJS = (mod) => 'local';
const esm = require('./esm.js');

module.exports = { v: esm.v, local: __toCommonJS(1) };
