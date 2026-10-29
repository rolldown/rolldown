import assert from 'node:assert';
import { a } from './dist/a.js';
import { b } from './dist/b.js';

assert.strictEqual(a, 'a:shared');
assert.strictEqual(b, 'b:shared');
