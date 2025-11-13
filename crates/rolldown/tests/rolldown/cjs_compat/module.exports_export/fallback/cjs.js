// CJS wrapper that requires the ESM without module.exports
const result = require('./esm.js');
module.exports = result;
