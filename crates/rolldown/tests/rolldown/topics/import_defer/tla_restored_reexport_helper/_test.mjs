import assert from 'node:assert';
import { log } from './dist/main.js';

// Tree shaking removes the unused re-export in `a.js`. With `moduleSideEffects: false`, it also
// removes the `log.push` calls of `a.js` and `b.js`. The init of `a.js` still calls the init of
// `b.js`, in the `__esmDepDone` form, because `b.js` has top-level await. The bundle must keep that
// runtime helper. `b.js` already finished, so the deferred access `ns.x` evaluates `a.js`
// synchronously.
assert.deepStrictEqual(log, ['main y=2 x=1']);
