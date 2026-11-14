import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';
import path from 'node:path';

// Before the fix, `expr` kept its source names: `require(aliased)` threw
// `ReferenceError: aliased is not defined`, and a renamed `Object` printed as `require(Object)`
// received the global `Object`.
const require = createRequire(import.meta.url);
const main = require('./dist/main.js');
assert.equal((await main.viaImport()).sep, path.sep);
assert.equal((await main.viaParam('node:path')).sep, path.sep);
assert.equal(main.cjsRoot.specifier, 'node:path');
assert.equal((await main.cjsRoot.load()).sep, path.sep);
