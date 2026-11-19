import assert from 'node:assert';
import { log, value } from './dist/main.js';

assert.strictEqual(value, 'd');
assert.deepStrictEqual(log, ['a', 'before read', 'm', 'd', 'after read', 'microtask']);
