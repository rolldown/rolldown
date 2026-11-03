import assert from 'assert';
import * as ns from './import.js';

assert.deepStrictEqual(Object.getOwnPropertyNames(ns), ['default']);
assert.equal(Object.prototype.toString.call(ns), '[object Module]');
