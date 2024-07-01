exports.foo = 123
import assert from 'node:assert'
import {foo} from './entry'

assert.equal(foo, undefined)
