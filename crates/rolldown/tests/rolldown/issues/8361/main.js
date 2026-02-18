import assert from 'node:assert'

// This tests that runtime helpers (__commonJSMin) are available when chunk
// optimization is enabled. The runtime module must not be merged into the
// entry chunk because it creates a circular dependency: the entry chunk
// imports CJS wrapper chunks, which import __commonJSMin from the runtime.
import './b.js'
import './a.js'

const cjs = require('./cjs.js')
assert.strictEqual(cjs, 42)
