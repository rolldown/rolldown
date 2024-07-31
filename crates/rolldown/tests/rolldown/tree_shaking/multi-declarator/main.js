import assert from 'node:assert'
import { foo, c  } from './foo'


assert.equal(foo, 'foo')
assert.equal(c, 1)
