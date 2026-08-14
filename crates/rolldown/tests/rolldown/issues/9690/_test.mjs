import assert from 'node:assert';
import { createRequire } from 'node:module';

// https://github.com/rolldown/rolldown/issues/9690
// `.ts` and `.tsx` entries with identical content (importing a default from an
// external CJS module under `"type": "module"`) must get identical node-mode
// CJS interop. Previously the `.tsx` entry lost the node-mode flag
// (`__toESM(mod)` instead of `__toESM(mod, 1)`) because `oxc_resolver` only
// resolves `package.json#type` for `.js`/`.ts`, so its default import resolved
// to `exports.default` instead of `module.exports` and `fn.div` blew up at
// runtime. Executing both bundles asserts the runtime behavior directly.
const require = createRequire(import.meta.url);

const fromTs = require('./dist/from-ts.js');
const fromTsx = require('./dist/from-tsx.js');

assert.strictEqual(fromTs.result, 'div-tag-result');
assert.strictEqual(fromTsx.result, 'div-tag-result');
