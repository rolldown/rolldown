import assert from 'node:assert';
import { a, b } from './dist/main';

assert.strictEqual(a, 100);
assert.strictEqual(b, 200);
