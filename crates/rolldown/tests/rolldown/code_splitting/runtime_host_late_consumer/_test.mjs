import assert from 'node:assert/strict';

// `logger.js` needs `__exportAll` only once its dynamic-entry facade is merged into its chunk.
// Entry `a` never loads `shared.js`, so the runtime must not be hosted in that chunk.
const { a } = await import('./dist/a.js');
assert.equal(a, 'log: a');
assert.equal(globalThis.sharedEvaluated, undefined);

const { b, lazyLog } = await import('./dist/b.js');
const { c } = await import('./dist/c.js');
assert.equal(await lazyLog(), 'log: b');
assert.equal(b.value, 'shared');
assert.equal(c, b);
assert.equal(globalThis.sharedEvaluated, true);
