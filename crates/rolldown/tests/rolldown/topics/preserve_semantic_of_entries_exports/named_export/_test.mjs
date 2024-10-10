import assert from 'node:assert'
import * as main from './dist/main.js'
assert.deepStrictEqual(main.foo, 'foo')
assert.deepStrictEqual(main.default, 'main')
