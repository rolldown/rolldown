// Companion to `cjs_shadowing_suffixed_wrapper_name`, covering the *unused-require* record.
//
// `./b/dup.cjs` is required for its side effect alone, so the record carries
// `ImportRecordMeta::IsRequireUnused`. That flag only drops the `__toCommonJS(ns)` half of the
// rewrite -- the finalizer still prints a wrapper call inside this closure. So the synthesized
// wrapper must still avoid the name of the author-local below (`require_dup$2`).
const require_dup = require('./a/dup.cjs');

// Not a require-local: it only has the name that the wrapper of `./b/dup.cjs` would take
// otherwise.
const require_dup$1 = 'local-string';

require('./b/dup.cjs');

// `module.exports` keeps both locals alive through tree shaking.
module.exports = { a: require_dup.value, local: require_dup$1 };
