import assert from 'node:assert';
import defer * as ns from './data.json';

assert.strictEqual(Object.prototype.toString.call(ns), '[object Deferred Module]');
assert.deepStrictEqual(Object.keys(ns), ['a', 'b', 'default']);
assert.strictEqual(ns.a, 1);
assert.deepStrictEqual(ns.default, { a: 1, b: 2 });
