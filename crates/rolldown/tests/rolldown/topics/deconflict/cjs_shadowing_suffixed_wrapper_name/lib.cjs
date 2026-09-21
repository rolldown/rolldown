// CJS dependency, so it renders inside a `__commonJSMin((exports, module) => { ... })` closure.
//
// The two importees share a basename, so rolldown derives `require_dup` for one wrapper and
// `require_dup$1` for the other. This file's author-locals are named exactly that -- the shape
// rolldown's own CJS output has (`require_<basename>` with a `$N` suffix for duplicates), which is
// how a rolldown-built package re-bundled by another rolldown build hits it.
//
// Pre-fix, `rename_cjs_locals_shadowing_referenced_chunk_bindings` skipped CommonJS importees
// outright. A different pass saved `require_dup`: `collect_chunk_scope_captured_names` in
// `deconflict_chunk_symbols.rs`. That pass stores wrappers' *pre-deconfliction* names, so it
// covers the un-suffixed name only. No pass saved `require_dup$1`. It collided with the
// deconflicted wrapper for `./b/dup.cjs`, and rolldown emitted the self-referential
// `const require_dup$1 = require_dup$1()` (issue #10792).
const require_dup = require('./a/dup.cjs');
const require_dup$1 = require('./b/dup.cjs');

// The snapshot treats the two locals differently. `require_dup` keeps its name and the wrapper
// moves to `require_dup$2`. `require_dup$1` becomes `require_dup$1$1`. The renamer never reserves
// CJS closure locals at chunk scope, so only the second local must move.
//
// `module.exports` keeps both locals alive through tree shaking.
module.exports = { a: require_dup.value, b: require_dup$1.value };
