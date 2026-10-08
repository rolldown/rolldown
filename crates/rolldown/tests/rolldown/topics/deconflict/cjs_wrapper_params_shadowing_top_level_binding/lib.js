// A CJS-wrapped module (it writes `exports`) that imports from an ESM module. Its top-level `this`
// gives its closure the parameters `(exports, module, this$1)`, so the imports, printed inside the
// closure under `esm.js`'s top-level names, must not be named `module` or `exports`.
import { module as m, exports as e } from './esm.js';
exports.value = [m, e];
exports.self = this === exports;
