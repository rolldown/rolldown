// CommonJS leaf. Rolldown derives its wrapper-function name from the filename:
// `require_isArrayLike`. `main.js` imports it eagerly, so its wrapper is hoisted
// into the entry chunk — a *different* chunk than `last.cjs`'s wrapper.
function isArrayLike(value) {
  return value != null && typeof value.length === 'number';
}
exports.isArrayLike = isArrayLike;
