import assert from 'node:assert'
import './dist/main.js'

assert.deepEqual(globalThis.result, ['foo', 'ready'])
