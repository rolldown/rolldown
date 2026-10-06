// Companion to `cjs_shadowing_suffixed_wrapper_name`, covering the *unused-require* record.
//
// `./b/dup.cjs` is required for its side effect alone, so the record carries
// `ImportRecordMeta::IsRequireUnused`. That flag only drops the `__toCommonJS(ns)` half of the
// rewrite -- the finalizer still emits the wrapper call `require_dup$1()`. So the author-local
// below must still be recognized as shadowing a chunk-root wrapper name.
const require_dup = require('./a/dup.cjs');

// Not a require-local: it only has to occupy the name deconfliction hands to `./b/dup.cjs`'s
// wrapper.
const require_dup$1 = 'local-string';

require('./b/dup.cjs');

// `module.exports` keeps both locals alive through tree shaking.
module.exports = { a: require_dup.value, local: require_dup$1 };
