// `barrel.js` re-exports these modules with `export *`, but no module imports their exports:
// `unused.js` has an export, `empty.js` has none, and the two `ambiguous-*.js` export the same name.
globalThis.out = require('./consumer.js').out;
