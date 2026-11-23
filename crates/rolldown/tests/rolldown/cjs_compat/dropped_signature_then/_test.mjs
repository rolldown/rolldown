import assert from 'node:assert/strict';

const e1 = await import('./dist/e1.js');
const e2 = await import('./dist/e2.js');
assert.equal(typeof e1.default.then, 'function');
assert.equal(e2.default, e1.default);
