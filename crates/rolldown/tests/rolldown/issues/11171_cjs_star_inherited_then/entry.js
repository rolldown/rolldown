import assert from 'node:assert/strict';

const ns = await import('./re.js');
assert.equal(typeof ns, 'object');
assert.ok(Object.hasOwn(ns, 'then'));
assert.equal(ns.then, undefined);
