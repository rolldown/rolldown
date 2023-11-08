import assert from 'node:assert'
import * as main from './dist/main.mjs'

assert.equal(main.a, 'a1')
assert.equal(main.b, 'b1')
assert.equal(main.c, 'c1')
assert.equal(main.d, 'd1')
assert.equal(main.e, 'e1')
assert.equal(main.a2, 'a2')
assert.equal(main.b2, 'b2')
assert.equal(main.c2, 'c2')
assert.equal(main.d2, 'd2')
assert.equal(main.e2, 'e2')
