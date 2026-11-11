import assert from 'node:assert';
import * as ns from './cjs.cjs';

assert.strictEqual(ns.default.foo, 'default.foo');
