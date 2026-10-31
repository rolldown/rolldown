import assert from 'node:assert/strict';

// `log.js` needs `__exportAll` only after facade elimination merges its dynamic entry into a
// common chunk. The runtime must not go into the chunk of `ns.js`. Only `c.js` and `d.js` load
// that chunk.
const { a } = await import('./dist/a.js');
assert.equal(a, 'log: a');
assert.equal(globalThis.nsEvaluated, undefined);

const { b } = await import('./dist/b.js');
assert.equal(await b(), 'log: b');
assert.equal(globalThis.nsEvaluated, undefined);

const { c } = await import('./dist/c.js');
const { d } = await import('./dist/d.js');
assert.equal(c.value, 'ns');
assert.equal(d.value, 'ns');
assert.equal(globalThis.nsEvaluated, true);
