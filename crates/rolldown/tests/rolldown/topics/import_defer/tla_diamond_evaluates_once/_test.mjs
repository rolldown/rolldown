import assert from 'node:assert';
import { log } from './dist/main.js';

// `a.js` imports `b.js` and then `c.js`, and `b.js` also imports `c.js`. The preparation of the
// deferred import walks from `a.js` into `b.js` first, and reaches `c.js` there. `c.js` must still
// evaluate one time.
assert.deepStrictEqual(log, ['c', 'b', 'a', 'main x=1']);
