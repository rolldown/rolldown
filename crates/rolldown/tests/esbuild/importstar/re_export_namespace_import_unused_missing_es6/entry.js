import assert from 'node:assert'
import {ns} from './foo'
assert.deepEqual(ns, {
  x: 123
})
