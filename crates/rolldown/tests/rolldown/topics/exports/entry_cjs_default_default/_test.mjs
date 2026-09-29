import assert from 'node:assert';
import { createRequire } from 'node:module';

// https://github.com/rolldown/rolldown/issues/11027
// `lib.js` is a CommonJS entry, so its chunk exports `module.exports` as its
// only (default) export. The runtime must not be merged into it: its helpers
// would be exported with further `module.exports =` statements that overwrite
// the entry's value.
const require = createRequire(import.meta.url);

const lib = require('./dist/lib.js');
assert.deepStrictEqual(lib, { value: 42 });

const main = require('./dist/main.js');
assert.strictEqual(main, lib);
