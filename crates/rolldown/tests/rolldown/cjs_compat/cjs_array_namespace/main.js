import assert from 'node:assert';
import * as ns from './array.cjs';
import * as named from './named.cjs';
import * as esModule from './es_module.cjs';
import * as proxy from './proxy.cjs';
import * as reexport from './reexport.js';
import * as revoked from './revoked.cjs';
import { length as namedLength } from './named_import.cjs';
import mixed, { length as mixedLength } from './mixed_import.cjs';
import { length as sharedLength } from './mixed_import.cjs';
import './node_mode.mjs';

// Indices and `length` are not exports, and the namespace does not inherit `Array.prototype`.
assert.deepStrictEqual(ns.default, [1, '2']);
assert.equal(Object.hasOwn(ns, '0'), false);
assert.equal(Object.hasOwn(ns, 'length'), false);
assert.notEqual(Object.getPrototypeOf(ns), Array.prototype);

// Named properties of the array are still exports. `2 ** 32 - 1` is a name, not an index.
assert.equal(named.foo, 'bar');
assert.equal(named.default.foo, 'bar');
assert.equal(named['4294967295'], 'not an index');
assert.deepStrictEqual(Object.keys(reexport).sort(), ['4294967295', 'foo']);

// A Proxy can list `length` before the indices, so the key order does not matter.
assert.equal(proxy.foo, 'bar');
assert.equal(Object.hasOwn(proxy, '0'), false);
assert.equal(Object.hasOwn(proxy, 'length'), false);

// `ownKeys` revokes this Proxy, so the copy must not touch it after the enumeration.
assert.deepStrictEqual(Object.keys(revoked), ['default']);

// Babel interop keeps an explicit `default`.
assert.equal(esModule.default, 'value');

// A named import reads `module.exports`, unless an import of the same module needs the namespace.
assert.equal(namedLength, 1);
assert.equal(mixedLength, undefined);
assert.equal(sharedLength, undefined);
assert.equal(mixed.length, 1);
