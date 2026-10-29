import assert from 'node:assert';
import { createRequire } from 'node:module';

// The bug (#9630): a CJS wrapper hoisted across a chunk boundary was imported
// under its un-deconflicted base name (`require_isArrayLike`), which an author's
// local binding of the same name then shadowed, emitting the self-shadowing
// `var require_isArrayLike = require_isArrayLike()`. The CJS variant covers the
// same cross-chunk wrapper reference without treating CJS's generated
// `const require_<chunk> = require(...)` binding as an ESM import binding.
const require = createRequire(import.meta.url);
const mod =
  globalThis.__configName === 'cjs' ? require('./dist/main.js') : await import('./dist/main.js');

assert.strictEqual(mod.eager, true);
assert.strictEqual(await mod.lazy(), 3);
