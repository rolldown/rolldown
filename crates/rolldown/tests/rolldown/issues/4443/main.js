import assert from 'node:assert'
// index.js
import './foo.js'

assert.strictEqual(globalThis.value, 'foo', 'globalThis.value should be "foo"')