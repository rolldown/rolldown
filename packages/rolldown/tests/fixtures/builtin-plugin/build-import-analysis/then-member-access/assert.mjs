// @ts-nocheck
import assert from 'node:assert';
import { a, promiseB, c, d } from './dist/main';

assert.strictEqual(a, 100, 'Pattern 1: await with arrow function should work');
assert.strictEqual(d, 100, 'Pattern 4: await with regular function should work');

// promiseB is a promise, so we need to await it
const b = await promiseB;
assert.strictEqual(b, 200, 'Pattern 2: without await should work');

assert.strictEqual(c, 300, 'Pattern 3: nested property access should work');

console.log('All tests passed!');
