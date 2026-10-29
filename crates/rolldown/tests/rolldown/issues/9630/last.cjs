// CommonJS module that requires the leaf. The critical detail (copied from
// es-toolkit's compiled `dist/compat/array/last.js`): the author's OWN local
// binding is literally named `require_isArrayLike` — the SAME name rolldown
// derives for the leaf's wrapper function. When both wrappers land in the same
// chunk, rolldown deconflicts (wrapper -> `require_isArrayLike$1`). When the
// leaf's wrapper lives in another chunk it is imported under its base name and
// the local binding then shadows it, emitting the self-shadowing
// `var require_isArrayLike = require_isArrayLike()`.
var require_isArrayLike = require('./isArrayLike.cjs');

function last(array) {
  if (!require_isArrayLike.isArrayLike(array)) return;
  return array[array.length - 1];
}

// CJS-format output derives a cross-chunk `require_<chunk>` binding from the
// imported wrapper's chunk. This source local catches regressions where ESM-only
// cross-chunk wrapper reservations push that generated binding to the same name.
var require_isArrayLike$2 = {};

exports.shadow = function () {
  return require_isArrayLike$2;
};
exports.last = last;
