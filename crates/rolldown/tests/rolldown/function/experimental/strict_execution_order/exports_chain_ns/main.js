import nodeAssert from 'node:assert'
import * as star from './proxy'

nodeAssert.equal(star.foo, 'foo')