import assert from 'node:assert';
import { default as countInDefault, inc, reset } from './dist/main.js';
import * as star from './dist/main.js';

reset()
assert.strictEqual(countInDefault, 0)
assert.strictEqual(star.default, countInDefault)
inc()
assert.strictEqual(countInDefault, 1)
assert.strictEqual(star.default, countInDefault)
