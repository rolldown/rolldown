import assert from 'node:assert'
import value, { '😈' as devil } from './foo.json'

assert.deepStrictEqual(value, {
  '😈': devil
})