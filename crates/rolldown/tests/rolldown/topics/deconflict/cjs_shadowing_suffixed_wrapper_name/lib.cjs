// CJS dependency, so it renders inside a `__commonJSMin((exports, module) => { ... })` closure.
//
// The two importees share a basename, so rolldown derives `require_dup` for one wrapper and
// `require_dup$1` for the other. This file's author-locals are named exactly that -- the shape
// rolldown's own CJS output has (`require_<basename>` with a `$N` suffix for duplicates), which is
// how a rolldown-built package re-bundled by another rolldown build hits it.
//
// Pre-fix, only the *un-suffixed* `require_dup` was recognized as shadowing a chunk-root wrapper,
// so `require_dup$1` kept its name and collided with the deconflicted wrapper for `./b/dup.cjs`,
// emitting the self-referential `const require_dup$1 = require_dup$1()` (issue #10792).
const require_dup = require('./a/dup.cjs');
const require_dup$1 = require('./b/dup.cjs');

// `module.exports` keeps both locals alive through tree shaking.
module.exports = { a: require_dup.value, b: require_dup$1.value };
