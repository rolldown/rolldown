import assert from 'node:assert';
import { log } from './dist/main.js';

// `a.js` and `b.js` form a cycle, and `b.js` waits at an `await`. `a.js` must wait for `b.js`, so
// `a.js` and the deferred read see the value that `b.js` sets after the `await`.
assert.deepStrictEqual(log, ['b start', 'b end', 'a sees set', 'main nsB.x=set']);
