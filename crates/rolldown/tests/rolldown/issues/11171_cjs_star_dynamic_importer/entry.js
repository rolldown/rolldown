import assert from 'node:assert/strict';

const cjs = await import('./cjs.cjs');
cjs.default.then = (resolve) => resolve('assimilated');
const ns = await import('./re.js');
assert.equal(typeof ns, 'object');
