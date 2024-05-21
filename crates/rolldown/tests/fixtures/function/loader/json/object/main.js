import assert from 'node:assert'
import value, { foo } from './foo.json'

assert.deepStrictEqual(value, {
  foo: foo
})