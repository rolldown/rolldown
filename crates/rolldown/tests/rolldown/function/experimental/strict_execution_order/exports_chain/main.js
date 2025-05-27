import nodeAssert from 'node:assert'
import { foo } from './proxy'

nodeAssert.equal(foo, 'foo')