import assert from 'node:assert';
import { order, value } from './dist/main.js';

assert.deepStrictEqual(order, ['x', 'y']);
assert.strictEqual(value, 'foo');
