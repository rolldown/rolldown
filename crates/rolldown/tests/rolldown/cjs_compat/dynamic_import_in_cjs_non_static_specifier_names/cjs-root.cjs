// The lowered `import()` prints `Object` inside this CJS closure, so the root binding `Object` gets
// a new name. The specifier of the `import()` must then print that new name.
var Object = process.env.TEST_IMPORT_PATH || 'node:path';
module.exports = { specifier: Object, load: () => import(Object) };
