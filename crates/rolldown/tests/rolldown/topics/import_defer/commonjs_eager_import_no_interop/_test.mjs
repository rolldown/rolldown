import assert from 'node:assert';
import { after, before, values } from './dist/main.js';

// The eager imports in `c.js` and `d.js` run both modules before the body of `main.js`. The
// deferred accesses then read the same modules, through `__toESM`.
assert.deepStrictEqual(before, ['x', 'y']);
assert.deepStrictEqual(values, ['x-foo', 'y-foo', 'y-foo', 'object']);
assert.deepStrictEqual(after, ['x', 'y']);
