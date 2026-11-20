import assert from 'node:assert';
import { later, seen, value } from './dist/main.js';

assert.strictEqual(value, 'd');
assert.strictEqual(seen, '[object Deferred Module]');
assert.strictEqual(later, 'd');
