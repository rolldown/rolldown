import assert from 'node:assert';
import { default as countInDefault, inc, reset } from './shared.js';
import * as star from './shared.js';

reset()
assert.strictEqual(countInDefault, 0)
assert.strictEqual(star.default, countInDefault)
inc()
assert.strictEqual(countInDefault, 1)
assert.strictEqual(star.default, countInDefault)
