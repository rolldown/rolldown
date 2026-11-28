import assert from 'node:assert';
import { log } from './dist/main.js';

// `r.js`, `p.js`, `x.js` and `c.js` form one cycle. `c.js` is not the cycle root, and it waits for
// `t.js`, which has top-level await. `p.js` and `x.js` import `c.js`, so they must wait for `c.js`,
// and the deferred read sees the value that `c.js` sets.
assert.deepStrictEqual(log, [
  't start',
  't end',
  'c start r=undefined t=set',
  'c end',
  'p start c=set',
  'p end',
  'x start c=set',
  'x end',
  'r start p=set x=set',
  'r end',
  'main sees ns.v=set',
]);
