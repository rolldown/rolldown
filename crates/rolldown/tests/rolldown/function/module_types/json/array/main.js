import assert from 'node:assert'
import value from './foo.json'
assert.deepStrictEqual(value, ['foo'])