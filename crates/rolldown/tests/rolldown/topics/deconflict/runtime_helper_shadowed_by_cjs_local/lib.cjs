// The local and the runtime helper for `require('./esm.js')` share the name `__requireESM` before deconfliction.
var __requireESM = (mod) => 'local';
const esm = require('./esm.js');

module.exports = { v: esm.v, local: __requireESM(1) };
