import assert from 'node:assert'
import value from './foo.json2'
assert.deepStrictEqual(value, ['foo'])