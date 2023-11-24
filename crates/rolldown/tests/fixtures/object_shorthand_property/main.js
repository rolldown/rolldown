import foo from './foo'
import assert from 'assert'

const value = { foo }

assert.strictEqual(value.foo, 'foo')