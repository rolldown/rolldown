const wasmPath = require('path').join(__dirname, 'rosu_pp_js_bg.wasm');

module.exports = {
  wasmPath,
  readPaths() {
    return [__dirname, __filename];
  },
};
