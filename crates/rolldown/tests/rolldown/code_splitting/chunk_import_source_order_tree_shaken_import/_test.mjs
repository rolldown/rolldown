import assert from 'node:assert/strict';

// `dead.js` imports `data.js`, but only the removed `unused` function reads `data`. That import is
// not in the output, so it must not move the chunk of `data.js` and `read.js` before the chunk of
// `init.js`.
const { name, value } = await import('./dist/a.js');
assert.equal(name, 'data');
assert.equal(value, 'ready');
