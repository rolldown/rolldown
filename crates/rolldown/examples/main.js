import assert from 'assert'
import { a as aJs } from './a'
const a = 'main.js'
const b = 'tesajtkfjeka'


function foo(a$1) {
  return [a$1, a, aJs, b]
}

assert.deepEqual(foo('foo'), ['foo', 'main.js', 'a.js'])
