import xyz from '@some-scope/xyz'
import foo from '@some-scope/xyz/bar'
import assert from 'node:assert'
export default 'index'
assert.equal(xyz, 'index')
assert.equal(foo, 'foo')
