import assert from 'node:assert';
import * as ns from './foo.js';
require('./foo.js');

assert.deepStrictEqual(Object.keys(ns), ['foo']);
assert.strictEqual(ns.foo, 123);
