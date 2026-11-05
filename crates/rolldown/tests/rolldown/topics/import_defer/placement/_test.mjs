import assert from 'node:assert';
import { order, value } from './dist/main.js';

assert.deepStrictEqual(order, ['d']);
assert.strictEqual(value, 'd');
