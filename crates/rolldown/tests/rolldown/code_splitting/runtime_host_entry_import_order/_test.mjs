import assert from 'node:assert/strict';

// Load c first: its namespace import initializes the global before common captures it.
// Moving the runtime out of the namespace chunk must not reverse these imports.
const { c, snapshot } = await import('./dist/c.js');
assert.equal(c.value, 'namespace');
assert.equal(snapshot, 'ready');
