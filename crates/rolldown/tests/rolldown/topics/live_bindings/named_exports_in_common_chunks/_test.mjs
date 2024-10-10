import assert from 'node:assert';
import { count, inc, reset } from './dist/main.js';
import * as star from './dist/main.js';

reset()
assert.strictEqual(count, 0)
assert.strictEqual(star.count, count)
inc()
assert.strictEqual(count, 1)
assert.strictEqual(star.count, count)
inc()
assert.strictEqual(count, 2)
assert.strictEqual(star.count, count)
