import assert from 'assert'
import { a as a2, b as b2, c as c2, d as d2, e as e2 } from './names.js'
export const [a] = ['a1']
export const { b } = { b: 'b1' }
export const { b: c } = { b: 'c1' }
export const { d = '' } = { d: 'd1' }
export const { d: e = '' } = { d: 'e1' }
export { a2, b2, c2, d2, e2 }

assert.equal(a, 'a1')
assert.equal(a2, 'a2')
assert.equal(b, 'b1')
assert.equal(b2, 'b2')
assert.equal(c, 'c1')
assert.equal(c2, 'c2')
assert.equal(d, 'd1')
assert.equal(d2, 'd2')
assert.equal(e, 'e1')
assert.equal(e2, 'e2')

