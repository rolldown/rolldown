import nodeAssert from 'node:assert'
import { value } from './foo.cjs'

nodeAssert.strictEqual(value, 'foo')