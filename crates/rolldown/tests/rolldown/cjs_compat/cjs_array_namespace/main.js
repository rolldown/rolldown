import assert from 'node:assert';
import * as ns from './array.cjs';

assert.deepStrictEqual(ns.default, [1, '2']);
assert.equal(Object.hasOwn(ns, '0'), false);
assert.equal(Object.hasOwn(ns, 'length'), false);
assert.notEqual(Object.getPrototypeOf(ns), Array.prototype);
