// `esm.js` has no used export, so this `import()` is dead. It is lowered to
// `Promise.resolve().then(() => Object.freeze(...))` inside this closure. The closure local
// `Object` must not capture that `Object`. Two reads keep the local alive.
var Object = require('node:os');
exports.os = [typeof Object.platform, typeof Object.arch];
import('./esm.js');
