import assert from 'node:assert';

// The bug (#9502) manifests as an eagerly-executed `init_*()` call in a chunk
// that fails to import it, so simply importing the entry throws a ReferenceError
// on broken output.
const { slot } = await import('./dist/tu.js');
assert.equal(slot(), Object.assign);
