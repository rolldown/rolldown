import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

// Entry `a` does not import `namespace.js`. The chunk of `logger.js` needs helpers, and it must not
// import the chunk of `namespace.js` to get them, or `a` runs the side effect of `namespace.js`.
// A fresh process, because the check below must load `common.js` after `namespace.js`.
const entry = JSON.stringify(new URL('./dist/a.js', import.meta.url).href);
const script = [
  `const { a } = await import(${entry});`,
  'console.log(JSON.stringify([a, globalThis.ready ?? null]));',
].join(' ');
const [a, ready] = JSON.parse(
  execFileSync(process.execPath, ['--input-type=module', '-e', script], { encoding: 'utf8' }),
);
assert.equal(a, 'unset unset');
assert.equal(ready, null);

// `dead.js` imports `marker.js`, which is in the common chunk, but only the removed `unused`
// function reads `data`. The output of the namespace chunk does not have that import. So the
// runtime must not go into the common chunk, else `namespace.js` imports it, and `common.js` runs
// first in `c`.
const { c, snapshot } = await import('./dist/c.js');
assert.equal(c.value, 'namespace');
assert.equal(snapshot, 'ready');
