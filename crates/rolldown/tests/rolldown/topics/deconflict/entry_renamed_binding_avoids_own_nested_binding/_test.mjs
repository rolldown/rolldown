import { strict as assert } from 'node:assert';
import { f } from './dist/main.js';

// Pre-fix, the renamed top-level `join` landed on `f`'s parameter `join$1`, so `f` returned
// `['param', 'param']`, with no error at all.
assert.deepEqual(f('param'), ['local-join', 'param']);
