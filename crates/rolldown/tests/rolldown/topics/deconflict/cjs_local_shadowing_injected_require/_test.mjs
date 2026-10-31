import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';
import path from 'node:path';

const require = createRequire(import.meta.url);
const lib = require('./dist/main.js');

assert.equal(lib.local, 'local');
assert.equal((await lib.load()).sep, path.sep);
