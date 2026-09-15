import assert from 'node:assert';
import { value } from './dist/a.js';
import { combined } from './dist/b.js';

assert.strictEqual(value, 'a');
assert.strictEqual(combined, 'b:a');
