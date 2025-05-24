import nodeAssert from 'node:assert'
import './no_side_effect'

nodeAssert.equal(globalThis.value, undefined, 'Unused no side effect module should be removed')