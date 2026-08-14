import assert from 'node:assert';
import * as ns from './data.json';

ns.default.search = 'overridden';
assert.strictEqual(ns.default.search, 'overridden');
