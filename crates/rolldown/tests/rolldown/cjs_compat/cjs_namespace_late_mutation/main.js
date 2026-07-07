import assert from 'node:assert';
import * as target from './target.cjs';
import './augment.cjs';

// `import * as` from a CommonJS module produces a namespace via `__toESM`,
// which snapshots the own properties of `module.exports` at the moment the
// wrapper is created — that is, BEFORE `augment.cjs` runs `target.added = ...`.
// So the property added later by the side-effect module is NOT visible as a
// named export on the namespace object.
//
// This matches esbuild. Rollup (+ @rollup/plugin-commonjs) would instead show
// `typeof target.added === 'function'` because it keeps reading from the live
// CJS exports object. Rolldown intentionally follows the esbuild semantics
// here: named exports should not be addable by other modules.
//
// See https://github.com/rolldown/rolldown/issues/9512
assert.equal(target.existing, 'existing');
assert.equal(typeof target.added, 'undefined');

// The `default` export still points at the live CommonJS exports object, so the
// late mutation IS observable through it. This is the documented workaround.
assert.equal(typeof target.default?.added, 'function');
