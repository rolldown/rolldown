import cjs from './cjs.js';
import * as ns from './cjs_ns.js';
import assert from 'node:assert';
import { name } from './flag.js';

// This should work - it's a read
assert.strictEqual(cjs.a, 'original');
assert.strictEqual(cjs.b, 'original');
assert.strictEqual(cjs.c, 'ab');

// Static member expression assignment (cjs.a = ...)
// Without the fix, this throws: Cannot set property a of #<Object> which has only a getter
cjs.a = 'new value a';
assert.strictEqual(cjs.a, 'new value a');

// Computed member expression assignment (cjs["b"] = ...)
cjs[name] = 'new value b';
assert.strictEqual(cjs['b'], 'new value b');

cjs.c = 'abcd';
assert.strictEqual(cjs.c, 'abcd');

// Namespace import mutation
assert.strictEqual(ns.a, 'n1');
ns.default.a = 'ns1';
assert.strictEqual(ns.a, 'ns1');
