import nodeAssert from 'node:assert'
import { star } from './ns-proxy-2'

nodeAssert.equal(star.foo, 'foo')