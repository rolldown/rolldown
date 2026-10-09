import assert from 'node:assert';

const { P, V, util } = await import('./dist/styles.js');
assert.equal(V, 'v18');
assert.equal(P(), 'v18');
assert.equal(util, 1);
// Static dependencies initialize before the entry body observes their side effects.
assert.equal(globalThis.__entry_body_saw_w, 'ran');
