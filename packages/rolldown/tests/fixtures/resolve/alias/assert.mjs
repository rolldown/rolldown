import assert from 'node:assert'
import { foo, ignored } from './dist/main.js'

assert.strictEqual(foo, 'foo')
assert.strictEqual(ignored, void 0)
