import assert from 'node:assert/strict';

// The common chunk has the entry bits of every runtime consumer, but the namespace chunk does not
// import it. If the runtime goes into the common chunk, `namespace.js` must import it, and then
// `common.js` runs first in `c`.
const { c, snapshot } = await import('./dist/c.js');
assert.equal(c.value, 'namespace');
assert.equal(snapshot, 'ready');
