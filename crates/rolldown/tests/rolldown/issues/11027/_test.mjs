import assert from 'node:assert';
import { createRequire } from 'node:module';

// https://github.com/rolldown/rolldown/issues/11027
// `lib.js` is a CommonJS entry, so its chunk exports `module.exports` as its
// only (default) export. Neither the runtime nor `shared.js` (an ESM module
// that both entries use) may be merged into it: their exports would be
// rendered as further `module.exports =` statements that overwrite the
// entry's value.
const require = createRequire(import.meta.url);

const lib = require('./dist/lib.js');
assert.deepStrictEqual(lib, { value: 42, shared: 'shared' });

const main = require('./dist/main.js');
assert.deepStrictEqual(main, { value: 42, shared: 'shared' });
