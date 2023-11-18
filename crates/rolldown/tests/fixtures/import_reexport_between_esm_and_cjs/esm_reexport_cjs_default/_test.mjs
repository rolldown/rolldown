import assert from 'node:assert'
import main from './dist/main.mjs'
assert.strictEqual(main, 'foo')
