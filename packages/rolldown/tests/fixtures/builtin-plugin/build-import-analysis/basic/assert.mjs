// @ts-nocheck
import assert from 'node:assert';
import { a, b, c, foo } from './dist/main';

assert.strictEqual(a, b);
assert.strictEqual(b, c);
assert.strictEqual(foo, c);
assert.strictEqual(foo, 100);
