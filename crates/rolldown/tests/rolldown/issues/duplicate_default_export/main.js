import assert from 'node:assert'

// Require
const required = require('./file.json')

// Dynamic Import
const dynamicRes = await import('./file.json')

// Verify both imports work and have the expected structure
assert.strictEqual(required.foo, 'bar')
assert.strictEqual(dynamicRes.default.foo, 'bar')
