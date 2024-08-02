import assert from 'node:assert'
import { count, inc, reset } from './shared.js';
import * as star from './shared.js';

reset()
assert.strictEqual(count, 0)
assert.strictEqual(star.count, count)
inc()
assert.strictEqual(count, 1)
assert.strictEqual(star.count, count)
inc()
assert.strictEqual(count, 2)
assert.strictEqual(star.count, count)
