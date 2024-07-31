import assert from 'node:assert'
import * as main from './dist/main.mjs'
assert.deepStrictEqual(main.foo, 'foo')
assert.deepStrictEqual(main.default, 'main')