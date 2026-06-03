import assert from 'node:assert';

// The bug (#9502) manifests as an eagerly-executed `init_*()` call in a chunk
// that fails to import it, so simply importing the entry throws a ReferenceError
// on broken output. The structural `init_*` scan lives in 9502_deep_chain.
const { slot } = await import('./dist/tu.js');
assert.equal(slot(), Object.assign);
