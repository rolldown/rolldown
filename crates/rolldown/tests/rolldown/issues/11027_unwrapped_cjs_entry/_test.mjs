import assert from 'node:assert';
import { createRequire } from 'node:module';

// https://github.com/rolldown/rolldown/issues/11027
// `lib.js` is a CommonJS entry that nothing imports, so it isn't wrapped and
// its chunk exports whatever it assigns to `module.exports`. Neither the
// runtime nor `shared.js` (which its dynamic import also uses) may be merged
// into it: their exports would be written to the `exports` object that
// `module.exports =` replaced, so `dynamic.js` couldn't read them.
const require = createRequire(import.meta.url);

const lib = require('./dist/lib.js');
assert.strictEqual(lib.shared, 'shared');

const dynamic = await lib.load();
assert.strictEqual(dynamic.value, 'shared');
