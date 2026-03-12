import assert from 'node:assert';
// With dce-only (the default), NODE_ENV should be 'development', not 'production'
assert.equal(process.env.NODE_ENV, 'development');
