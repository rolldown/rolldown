import assert from 'node:assert/strict';
import { eager, keys, lazy } from './dist/main.js';

const eagerId = './dir/a"b.eager.js';
const lazyId = './dir/a"b.lazy.js';
const keysId = './dir/a"b.keys.js';

assert.strictEqual(eager[eagerId].default, 42);
assert.strictEqual((await lazy[lazyId]()).default, 42);
assert.deepStrictEqual(keys, [keysId]);
