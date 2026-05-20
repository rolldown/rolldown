// User-authored require of a default-only ESM module. Expected shape
// (unchanged by #9263 fix): the local binds to the namespace object
// `{ default: <fn>, __esModule: true }`, not the function itself —
// matching esbuild's and webpack's `interop: false` default behavior.
const dep = require('./dep.mjs');

if (typeof dep === 'function') {
  throw new Error('regression: user-authored default-only ESM was unwrapped to .default');
}
if (typeof dep.default !== 'function' || dep.default() !== 42) {
  throw new Error('expected dep.default to be a callable returning 42');
}
