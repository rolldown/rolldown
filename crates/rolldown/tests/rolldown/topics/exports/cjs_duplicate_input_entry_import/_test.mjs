import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { v } from './user.mjs';

const require = createRequire(import.meta.url);
const b = require('./dist/b.js');
const a = require('./dist/a.js');
assert.deepStrictEqual(a, require('./lib.cjs'));
assert.strictEqual(b, a);
assert.strictEqual(require('./dist/u.js').v, v);
