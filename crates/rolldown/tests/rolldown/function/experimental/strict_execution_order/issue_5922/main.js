import assert from 'node:assert'
import * as ns from './heavy/index';
assert.strictEqual(ns.H01, true);
assert.strictEqual(globalThis.foo, 'foo');
