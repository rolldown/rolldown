import assert from 'node:assert/strict';

// `dead.js` imports `marker.js`, which is in the common chunk, but only the removed `unused`
// function reads `data`. The output of the namespace chunk does not have that import. So the
// runtime must not go into the common chunk, else `namespace.js` imports it, and `common.js` runs
// first in `c`.
const { c, snapshot } = await import('./dist/c.js');
assert.equal(c.value, 'namespace');
assert.equal(snapshot, 'ready');
