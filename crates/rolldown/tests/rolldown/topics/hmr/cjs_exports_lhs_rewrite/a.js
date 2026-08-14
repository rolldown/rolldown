// Covers the CJS idiom `exports = module.exports = {}` — both `exports` and
// `module` appear as bare identifiers on an AssignmentTarget LHS. Under the
// HMR wrapper `function(__rolldown_exports__, __rolldown_module__, …)` those
// bare identifiers have no enclosing binding, so they must be rewritten to the
// wrapper-parameter names. Without the rewrite the HMR patch throws
// `ReferenceError: exports is not defined` at runtime.
exports = module.exports = {};
exports.value = 'v1';
module.exports.other = 'o1';

console.log(exports.value, exports.other);

import.meta.hot.accept();
