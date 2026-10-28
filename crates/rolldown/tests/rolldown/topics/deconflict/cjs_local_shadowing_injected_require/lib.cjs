// With `dynamicImportInCjs: false`, the external `import()` below is lowered to
// `Promise.resolve().then(() => __toESM(require("node:path")))`, printed inside this module's
// `__commonJSMin` closure. The closure-local `require` must not capture that injected `require`.
var require = () => 'local';

exports.local = require();
exports.load = () => import('node:path');
