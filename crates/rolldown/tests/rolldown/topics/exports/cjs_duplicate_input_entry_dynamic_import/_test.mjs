import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { load } from './user.mjs';

const require = createRequire(import.meta.url);
const b = require('./dist/b.js');
const a = require('./dist/a.js');
assert.deepStrictEqual(a, require('./lib.cjs'));
assert.strictEqual(b, a);
const sourceNamespace = await load();
const namespace = await require('./dist/u.js').load();
assert.deepStrictEqual(namespace.default, sourceNamespace.default);
assert.strictEqual(namespace.default, a);
