// Printed inside a `__commonJSMin` closure, so these are closure locals. The unused
// `import('./pure.js')` is lowered to `Promise.resolve().then(() => Object.freeze(...))`.
var Promise = () => 'local';
var Object = () => 'object';
exports.local = Promise();
exports.object = Object();
exports.load = () => import('./dep.js');
import('./pure.js');
