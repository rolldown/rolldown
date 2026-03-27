if (process.env.USE_ESM) {
  module.exports = require('./esm-impl.mjs');
} else {
  module.exports = require('./cjs-impl.js');
}
