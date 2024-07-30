import assert from 'node:assert';
import { count, inc, reset } from './dist/main.mjs';
import * as star from './dist/main.mjs';

reset()
assert.strictEqual(count, 0)
assert.strictEqual(star.count, count)
inc()
assert.strictEqual(count, 1)
assert.strictEqual(star.count, count)
inc()
assert.strictEqual(count, 2)
assert.strictEqual(star.count, count)
