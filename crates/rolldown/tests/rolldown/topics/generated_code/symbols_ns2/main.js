import assert from 'assert';
import * as ns from './a.js';

assert.deepStrictEqual(Object.getOwnPropertyNames(ns), ['ns']);
assert.deepStrictEqual(Object.getOwnPropertyNames(ns.ns), ['a']);
assert.equal(Object.prototype.toString.call(ns), '[object Module]');
assert.equal(Object.prototype.toString.call(ns.ns), '[object Module]');
